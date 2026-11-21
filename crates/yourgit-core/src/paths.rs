use std::path::{Path, PathBuf};

use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub workspace_dir: PathBuf,
    pub db_path: PathBuf,
    pub logs_dir: PathBuf,
    pub staging_dir: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self, AppError> {
        let data_dir = env_path("YOURGIT_DATA_DIR")
            .or_else(|| dirs::data_dir().map(|dir| dir.join("YourGit")))
            .ok_or_else(|| {
                AppError::new(
                    "YOURGIT-2001",
                    "Diretório de dados não encontrado",
                    "Não foi possível determinar a pasta de dados do usuário.",
                )
            })?;

        let cache_dir = env_path("YOURGIT_CACHE_DIR")
            .or_else(|| dirs::cache_dir().map(|dir| dir.join("YourGit")))
            .unwrap_or_else(|| data_dir.join("cache"));

        let workspace_dir = env_path("YOURGIT_WORKSPACE")
            .or_else(|| dirs::home_dir().map(|dir| dir.join("YourGit").join("projects")))
            .unwrap_or_else(|| data_dir.join("projects"));

        Ok(Self::new(data_dir, cache_dir, workspace_dir))
    }

    pub fn new(data_dir: PathBuf, cache_dir: PathBuf, workspace_dir: PathBuf) -> Self {
        Self {
            db_path: data_dir.join("yourgit.db"),
            logs_dir: data_dir.join("logs"),
            staging_dir: cache_dir.join("staging"),
            data_dir,
            cache_dir,
            workspace_dir,
        }
    }

    pub fn ensure(&self) -> Result<(), AppError> {
        for dir in [
            &self.data_dir,
            &self.cache_dir,
            &self.cache_dir.join("downloads"),
            &self.workspace_dir,
            &self.logs_dir,
            &self.staging_dir,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn project_dir(&self, owner: &str, repo: &str) -> PathBuf {
        self.workspace_dir.join(owner).join(repo)
    }

    pub fn is_inside_workspace(&self, path: &Path) -> bool {
        let workspace = self
            .workspace_dir
            .canonicalize()
            .unwrap_or_else(|_| self.workspace_dir.clone());
        let candidate = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        candidate.starts_with(workspace)
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}
