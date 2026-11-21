use std::fs;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::domain::{Project, ProjectDetail};
use crate::download::download_archive;
use crate::error::AppError;
use crate::inspect::inspect_repository;
use crate::now;
use crate::paths::AppPaths;
use crate::source::parse_github_url;
use crate::store::Store;

/// Baixa, extrai, inspeciona e instala um repositório público.
///
/// O código nunca é executado nesta etapa.
pub fn install_from_url(
    paths: &AppPaths,
    store: &Store,
    url: &str,
    progress: impl Fn(&str),
) -> Result<ProjectDetail, AppError> {
    let source = parse_github_url(url)?;
    progress("Baixando repositório do GitHub...");

    let zip_path = download_archive(&source, &paths.cache_dir)?;

    let staging = paths
        .staging_dir
        .join(format!("install-{}", Uuid::new_v4()));
    fs::create_dir_all(&staging)?;

    let mut installed_path: Option<PathBuf> = None;
    let result = (|| -> Result<ProjectDetail, AppError> {
        progress("Extraindo arquivos com validação de segurança...");
        crate::archive::extract_zip_safe(&zip_path, &staging)?;

        progress("Detectando stack e montando o plano...");
        let plan = inspect_repository(&staging, source.subdir.as_deref().unwrap_or("."))?;

        let final_path = paths.project_dir(&source.owner, &source.repo);
        if final_path.exists() {
            return Err(AppError::new(
                "YOURGIT-4004",
                "Projeto já existe",
                format!(
                    "A pasta {} já existe. Remova o projeto antigo ou escolha outro workspace.",
                    final_path.display()
                ),
            ));
        }
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }

        progress("Movendo para o workspace...");
        move_dir(&staging, &final_path)?;
        installed_path = Some(final_path.clone());

        let timestamp = now();
        let project = Project {
            id: Uuid::new_v4().to_string(),
            name: source.repo.clone(),
            owner: Some(source.owner.clone()),
            repo: Some(source.repo.clone()),
            source_url: source.canonical_url(),
            requested_ref: source.git_ref.clone(),
            resolved_commit: None,
            subdir: plan.workdir.clone(),
            local_path: final_path.to_string_lossy().to_string(),
            state: "ready".into(),
            deps_ready: false,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        };

        store.insert_project_with_plan(&project, &plan)?;
        progress("Projeto instalado. Revise o plano antes de executar.");

        Ok(ProjectDetail {
            project,
            plan: Some(plan),
            runs: Vec::new(),
        })
    })();

    let _ = fs::remove_file(&zip_path);
    let _ = fs::remove_dir_all(&staging);

    if result.is_err() {
        if let Some(path) = installed_path {
            if path.exists() {
                let _ = fs::remove_dir_all(path);
            }
        }
    }

    result
}

pub fn load_detail(store: &Store, project_id: &str) -> Result<ProjectDetail, AppError> {
    let project = store.get_project(project_id)?;
    let plan = store.get_plan(project_id)?;
    let runs = store.list_runs(project_id, 25)?;
    Ok(ProjectDetail {
        project,
        plan,
        runs,
    })
}

pub fn remove_project(
    paths: &AppPaths,
    store: &Store,
    project_id: &str,
    delete_files: bool,
) -> Result<(), AppError> {
    let project = store.get_project(project_id)?;

    if delete_files {
        let path = PathBuf::from(&project.local_path);
        if path.exists() {
            if !paths.is_inside_workspace(&path) {
                return Err(AppError::new(
                    "YOURGIT-4005",
                    "Remoção bloqueada",
                    "A pasta do projeto está fora do workspace configurado.",
                )
                .with_hint("Remova os arquivos manualmente se tiver certeza."));
            }
            fs::remove_dir_all(&path)?;
        }
    }

    if !project_id.contains('/') && !project_id.contains('\\') && !project_id.contains("..") {
        let _ = fs::remove_dir_all(paths.logs_dir.join(project_id));
    }

    store.delete_project(project_id)?;
    Ok(())
}

fn move_dir(source: &Path, destination: &Path) -> Result<(), AppError> {
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_dir_all(source, destination)?;
            fs::remove_dir_all(source)?;
            Ok(())
        }
    }
}

fn copy_dir_all(source: &Path, destination: &Path) -> Result<(), AppError> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if file_type.is_symlink() {
            return Err(AppError::new(
                "YOURGIT-1005",
                "Link simbólico inesperado",
                "A instalação contém um link simbólico e não pode continuar.",
            ));
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
