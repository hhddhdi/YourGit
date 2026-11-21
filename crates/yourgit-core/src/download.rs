use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use uuid::Uuid;

use crate::error::AppError;
use crate::source::GitHubSource;

pub const MAX_ARCHIVE_BYTES: u64 = 1024 * 1024 * 1024;

pub fn archive_url(source: &GitHubSource) -> String {
    let reference = source.git_ref.as_deref().unwrap_or("HEAD");
    format!(
        "https://codeload.github.com/{}/{}/zip/{}",
        source.owner, source.repo, reference
    )
}

/// Baixa o zip do GitHub para o cache. Retorna o caminho do `.zip`.
pub fn download_archive(source: &GitHubSource, cache_dir: &Path) -> Result<PathBuf, AppError> {
    let downloads = cache_dir.join("downloads");
    std::fs::create_dir_all(&downloads)?;

    let id = Uuid::new_v4();
    let partial = downloads.join(format!("{id}.zip.part"));
    let final_path = downloads.join(format!("{id}.zip"));

    let result = (|| -> Result<PathBuf, AppError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("YourGit/0.1 (desktop app)")
            .timeout(Duration::from_secs(900))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()?;

        let url = archive_url(source);
        let mut response = client.get(&url).send()?;

        match response.status().as_u16() {
            401 | 403 => {
                return Err(AppError::new(
                    "YOURGIT-1002",
                    "Repositório privado",
                    "O GitHub negou o acesso a este repositório.",
                )
                .with_hint("Repositórios privados entram na v0.2 com login pelo GitHub."));
            }
            404 => {
                return Err(AppError::new(
                    "YOURGIT-1002",
                    "Repositório não encontrado",
                    "Confira o proprietário, o nome do repositório e a branch.",
                ));
            }
            status if !(200..300).contains(&status) => {
                return Err(AppError::new(
                    "YOURGIT-1002",
                    "Download falhou",
                    format!("O GitHub respondeu com HTTP {status}."),
                )
                .retryable());
            }
            _ => {}
        }

        let total = response.content_length();
        if total.unwrap_or(0) > MAX_ARCHIVE_BYTES {
            return Err(AppError::new(
                "YOURGIT-1004",
                "Repositório grande demais",
                "O arquivo excede o limite de 1 GB do MVP.",
            ));
        }

        let mut file = File::create(&partial)?;
        let mut buffer = [0u8; 64 * 1024];
        let mut downloaded = 0u64;

        loop {
            let read = response.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            downloaded = downloaded.saturating_add(read as u64);
            if downloaded > MAX_ARCHIVE_BYTES {
                return Err(AppError::new(
                    "YOURGIT-1004",
                    "Repositório grande demais",
                    "O download excedeu o limite de 1 GB do MVP.",
                ));
            }
            file.write_all(&buffer[..read])?;
        }
        file.flush()?;
        drop(file);

        std::fs::rename(&partial, &final_path)?;
        Ok(final_path)
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&partial);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_archive_url_for_default_branch() {
        let source = GitHubSource {
            owner: "owner".into(),
            repo: "repo".into(),
            git_ref: None,
            subdir: None,
        };
        assert_eq!(
            archive_url(&source),
            "https://codeload.github.com/owner/repo/zip/HEAD"
        );
    }

    #[test]
    fn builds_archive_url_for_ref() {
        let source = GitHubSource {
            owner: "owner".into(),
            repo: "repo".into(),
            git_ref: Some("v1.2.3".into()),
            subdir: None,
        };
        assert_eq!(
            archive_url(&source),
            "https://codeload.github.com/owner/repo/zip/v1.2.3"
        );
    }
}
