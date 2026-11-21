use std::fs::File;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use zip::ZipArchive;

use crate::error::AppError;

const MAX_ENTRIES: usize = 100_000;
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct ExtractStats {
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug)]
struct EntryMeta {
    name: String,
    is_dir: bool,
    is_symlink: bool,
}

/// Extrai um zip do GitHub para `dest`, removendo a pasta raiz do arquivo.
///
/// Proteções: zip-slip, caminhos absolutos, prefixos de drive, symlinks,
/// nomes reservados do Windows, ADS e limites de tamanho/quantidade.
pub fn extract_zip_safe(zip_path: &Path, dest: &Path) -> Result<ExtractStats, AppError> {
    std::fs::create_dir_all(dest)?;

    let (entries, strip_root) = inspect_entries(zip_path)?;
    let mut archive = ZipArchive::new(File::open(zip_path)?)?;
    let mut files = 0usize;
    let mut bytes = 0u64;

    for (index, meta) in entries.iter().enumerate() {
        if meta.is_dir {
            continue;
        }
        if meta.is_symlink {
            return Err(unsafe_archive(
                "O arquivo contém um link simbólico, que não é permitido.",
            ));
        }

        let mut entry = archive.by_index(index)?;
        let relative = sanitize_zip_path(&meta.name)?;
        let relative = if let Some(root) = strip_root.as_deref() {
            relative
                .strip_prefix(root)
                .map(Path::to_path_buf)
                .unwrap_or(relative)
        } else {
            relative
        };

        if relative.as_os_str().is_empty() {
            continue;
        }

        let output = dest.join(&relative);
        if !output.starts_with(dest) {
            return Err(unsafe_archive(
                "O arquivo tentou escrever fora da pasta de destino.",
            ));
        }
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if entry.size() > MAX_FILE_BYTES {
            return Err(unsafe_archive(
                "Um arquivo do repositório excede o limite de 512 MB.",
            ));
        }

        let mut file = File::create(&output)?;
        let mut limited = (&mut entry).take(MAX_FILE_BYTES + 1);
        let copied = io::copy(&mut limited, &mut file)?;
        if copied > MAX_FILE_BYTES {
            return Err(unsafe_archive("Um arquivo descompactado é grande demais."));
        }
        bytes = bytes.saturating_add(copied);
        if bytes > MAX_TOTAL_BYTES {
            return Err(unsafe_archive(
                "O conteúdo descompactado excede o limite de 2 GB.",
            ));
        }
        files += 1;
    }

    Ok(ExtractStats { files, bytes })
}

fn inspect_entries(
    zip_path: &Path,
) -> Result<(Vec<EntryMeta>, Option<std::ffi::OsString>), AppError> {
    let mut archive = ZipArchive::new(File::open(zip_path)?)?;
    if archive.len() > MAX_ENTRIES {
        return Err(unsafe_archive("O arquivo contém entradas demais."));
    }

    let mut entries = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let name = entry.name().to_string();
        let is_dir = entry.is_dir() || name.ends_with('/');
        let is_symlink = entry
            .unix_mode()
            .map(|mode| mode & 0o170000 == 0o120000)
            .unwrap_or(false);
        entries.push(EntryMeta {
            name,
            is_dir,
            is_symlink,
        });
    }
    drop(archive);

    let mut roots: Vec<std::ffi::OsString> = Vec::new();
    let mut strip = true;
    for entry in &entries {
        let relative = sanitize_zip_path(&entry.name)?;
        let mut components = relative.components();
        let Some(Component::Normal(root)) = components.next() else {
            strip = false;
            continue;
        };
        if !roots.iter().any(|existing| existing == root) {
            roots.push(root.to_os_string());
        }
        if components.next().is_none() && !entry.is_dir {
            strip = false;
        }
    }

    let strip_root = if strip && roots.len() == 1 {
        roots.into_iter().next()
    } else {
        None
    };

    Ok((entries, strip_root))
}

fn sanitize_zip_path(name: &str) -> Result<PathBuf, AppError> {
    let normalized = name.replace('\\', "/");
    if normalized.trim().is_empty() {
        return Err(unsafe_archive("O arquivo contém um caminho vazio."));
    }
    if normalized.starts_with('/') {
        return Err(unsafe_archive("O arquivo contém um caminho absoluto."));
    }

    let mut result = PathBuf::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_string_lossy();
                if part.is_empty()
                    || part == "."
                    || part == ".."
                    || part.contains(':')
                    || part.chars().any(char::is_control)
                    || is_windows_reserved(&part)
                {
                    return Err(unsafe_archive(
                        "O arquivo contém um nome de caminho não permitido.",
                    ));
                }
                result.push(part.as_ref());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(unsafe_archive(
                    "O arquivo contém um caminho não permitido (path traversal).",
                ));
            }
        }
    }

    if result.as_os_str().is_empty() {
        return Err(unsafe_archive("O arquivo contém um caminho vazio."));
    }
    Ok(result)
}

fn is_windows_reserved(part: &str) -> bool {
    let stem = part
        .split('.')
        .next()
        .unwrap_or(part)
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0')
}

fn unsafe_archive(message: &str) -> AppError {
    AppError::new("YOURGIT-1005", "Arquivo inseguro", message)
        .with_hint("O download foi interrompido e nenhuma pasta foi instalada.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn write_zip(path: &Path, entries: &[(&str, &[u8], u32)]) {
        let file = File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (name, content, mode) in entries {
            let options = SimpleFileOptions::default().unix_permissions(*mode);
            zip.start_file(*name, options).unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn extracts_and_strips_root_folder() {
        let temp = tempfile::tempdir().unwrap();
        let zip_path = temp.path().join("repo.zip");
        write_zip(
            &zip_path,
            &[
                ("repo-main/README.md", b"hello", 0o100644),
                ("repo-main/src/main.js", b"console.log(1)", 0o100644),
            ],
        );
        let dest = temp.path().join("out");
        let stats = extract_zip_safe(&zip_path, &dest).unwrap();
        assert_eq!(stats.files, 2);
        assert!(dest.join("README.md").is_file());
        assert!(dest.join("src/main.js").is_file());
        assert!(!dest.join("repo-main").exists());
    }

    #[test]
    fn rejects_zip_slip() {
        let temp = tempfile::tempdir().unwrap();
        let zip_path = temp.path().join("evil.zip");
        write_zip(&zip_path, &[("../evil.txt", b"boom", 0o100644)]);
        let dest = temp.path().join("out");
        assert!(extract_zip_safe(&zip_path, &dest).is_err());
        assert!(!temp.path().join("evil.txt").exists());
    }

    #[test]
    fn rejects_symlink() {
        let temp = tempfile::tempdir().unwrap();
        let zip_path = temp.path().join("link.zip");
        let file = File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default().unix_permissions(0o777);
        zip.add_symlink("repo-main/link", "/etc/passwd", options)
            .unwrap();
        zip.finish().unwrap();
        let dest = temp.path().join("out");
        assert!(extract_zip_safe(&zip_path, &dest).is_err());
    }

    #[test]
    fn rejects_windows_reserved_name() {
        let temp = tempfile::tempdir().unwrap();
        let zip_path = temp.path().join("reserved.zip");
        write_zip(&zip_path, &[("repo-main/CON", b"x", 0o100644)]);
        let dest = temp.path().join("out");
        assert!(extract_zip_safe(&zip_path, &dest).is_err());
    }
}
