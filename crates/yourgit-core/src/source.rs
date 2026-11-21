use percent_encoding::percent_decode_str;
use url::Url;

use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubSource {
    pub owner: String,
    pub repo: String,
    pub git_ref: Option<String>,
    pub subdir: Option<String>,
}

impl GitHubSource {
    pub fn canonical_url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.repo)
    }
}

/// Aceita URLs HTTPS, SSH e caminhos do GitHub.
pub fn parse_github_url(input: &str) -> Result<GitHubSource, AppError> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err(invalid_url("A URL está vazia."));
    }

    let normalized = if let Some(rest) = raw.strip_prefix("git@github.com:") {
        format!("https://github.com/{}", rest.trim_end_matches(".git"))
    } else if let Some(rest) = raw.strip_prefix("github.com/") {
        format!("https://github.com/{}", rest)
    } else if let Some(rest) = raw.strip_prefix("www.github.com/") {
        format!("https://github.com/{}", rest)
    } else {
        raw.to_string()
    };

    let url = Url::parse(&normalized).map_err(|_| invalid_url("Formato de URL inválido."))?;
    if url.scheme() != "https" && url.scheme() != "http" {
        return Err(invalid_url("Use uma URL http(s) do GitHub."));
    }

    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if host != "github.com" && host != "www.github.com" {
        return Err(invalid_url(
            "Por enquanto o YourGit aceita apenas repositórios do github.com.",
        ));
    }

    let segments: Vec<String> = url
        .path_segments()
        .map(|parts| {
            parts
                .filter(|part| !part.is_empty())
                .map(|part| {
                    percent_decode_str(part)
                        .decode_utf8_lossy()
                        .trim()
                        .to_string()
                })
                .collect()
        })
        .unwrap_or_default();

    if segments.len() < 2 {
        return Err(invalid_url(
            "A URL precisa ter o formato github.com/proprietário/repositório.",
        ));
    }

    let owner = segments[0].clone();
    let repo = segments[1].trim_end_matches(".git").to_string();
    validate_name(&owner, "proprietário")?;
    validate_name(&repo, "repositório")?;

    let mut git_ref = None;
    let mut subdir = None;

    if segments.len() >= 3 {
        match segments[2].as_str() {
            "tree" => {
                if segments.len() < 4 {
                    return Err(invalid_url("A URL /tree/ não informa uma branch ou tag."));
                }
                git_ref = Some(validate_ref(&segments[3])?);
                if segments.len() > 4 {
                    subdir = Some(sanitize_subdir(&segments[4..].join("/"))?);
                }
            }
            "releases" => {
                if segments.get(3).map(String::as_str) != Some("tag") || segments.len() < 5 {
                    return Err(invalid_url(
                        "Use uma URL de release no formato /releases/tag/<tag>.",
                    ));
                }
                git_ref = Some(validate_ref(&segments[4])?);
            }
            "commit" => {
                if segments.len() < 4 {
                    return Err(invalid_url("A URL /commit/ não informa um commit."));
                }
                git_ref = Some(validate_ref(&segments[3])?);
            }
            "blob" | "issues" | "pull" | "pulls" | "actions" | "settings" => {
                return Err(invalid_url(
                    "Essa URL aponta para uma página do GitHub, não para o repositório.",
                ));
            }
            _ => {
                return Err(invalid_url(
                    "Caminho do GitHub não suportado. Use a URL raiz ou /tree/<ref>/<pasta>.",
                ));
            }
        }
    }

    Ok(GitHubSource {
        owner,
        repo,
        git_ref,
        subdir,
    })
}

fn invalid_url(message: &str) -> AppError {
    AppError::new("YOURGIT-1001", "URL inválida", message).with_hint(
        "Exemplo: https://github.com/proprietario/repositorio ou github.com/proprietario/repositorio",
    )
}

fn validate_name(value: &str, label: &str) -> Result<(), AppError> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.starts_with('-')
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(invalid_url(&format!("Nome de {label} inválido.")));
    }
    Ok(())
}

fn validate_ref(value: &str) -> Result<String, AppError> {
    let decoded = value.trim();
    if decoded.is_empty()
        || decoded.contains("..")
        || decoded.starts_with('/')
        || decoded.ends_with('/')
        || !decoded
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
    {
        return Err(invalid_url("Branch, tag ou commit inválido."));
    }
    Ok(decoded.to_string())
}

pub fn sanitize_subdir(value: &str) -> Result<String, AppError> {
    let normalized = value.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains(':') || part.chars().any(char::is_control) {
            return Err(AppError::new(
                "YOURGIT-1005",
                "Caminho inválido",
                "A subpasta informada contém um caminho não permitido.",
            ));
        }
        parts.push(part);
    }
    if parts.is_empty() {
        Ok(".".to_string())
    } else {
        Ok(parts.join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_https() {
        let source = parse_github_url("https://github.com/owner/repo").unwrap();
        assert_eq!(source.owner, "owner");
        assert_eq!(source.repo, "repo");
        assert_eq!(source.git_ref, None);
        assert_eq!(source.subdir, None);
    }

    #[test]
    fn parses_git_suffix_and_ssh() {
        let source = parse_github_url("git@github.com:owner/repo.git").unwrap();
        assert_eq!(source.owner, "owner");
        assert_eq!(source.repo, "repo");
    }

    #[test]
    fn parses_tree_and_subdir() {
        let source = parse_github_url("https://github.com/owner/repo/tree/main/apps/web").unwrap();
        assert_eq!(source.git_ref.as_deref(), Some("main"));
        assert_eq!(source.subdir.as_deref(), Some("apps/web"));
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(parse_github_url("https://github.com/owner/repo/tree/main/../../etc").is_err());
    }

    #[test]
    fn rejects_non_github_host() {
        assert!(parse_github_url("https://gitlab.com/owner/repo").is_err());
    }

    #[test]
    fn parses_release_tag() {
        let source = parse_github_url("https://github.com/owner/repo/releases/tag/v1.2.3").unwrap();
        assert_eq!(source.git_ref.as_deref(), Some("v1.2.3"));
    }
}
