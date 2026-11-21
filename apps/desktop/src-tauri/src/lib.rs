mod runner;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use yourgit_core::domain::{LogLine, Project, ProjectDetail, RunInfo, RunRecord};
use yourgit_core::error::AppError;
use yourgit_core::installer::{
    install_from_url, load_detail, remove_project as remove_project_files,
};
use yourgit_core::now;
use yourgit_core::paths::AppPaths;
use yourgit_core::store::Store;

use runner::{RunControl, RunRegistry, RunStatusEvent};

#[derive(Clone)]
struct AppState {
    paths: AppPaths,
    store: Store,
    runs: RunRegistry,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    workspace_dir: String,
    data_dir: String,
    docker_available: bool,
    docker_version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DockerStatus {
    available: bool,
    version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    message: String,
}

async fn blocking<T, F>(task: F) -> Result<T, AppError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| AppError::internal(format!("Tarefa em segundo plano falhou: {error}")))?
}

fn docker_status_inner() -> DockerStatus {
    let mut command = Command::new("docker");
    command.args(["version", "--format", "{{.Server.Version}}"]);
    runner::hide_window(&mut command);
    match command.output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if version.is_empty() {
                DockerStatus {
                    available: false,
                    version: None,
                }
            } else {
                DockerStatus {
                    available: true,
                    version: Some(version),
                }
            }
        }
        _ => DockerStatus {
            available: false,
            version: None,
        },
    }
}

#[tauri::command]
async fn app_info(state: State<'_, AppState>) -> Result<AppInfo, AppError> {
    let state = state.inner().clone();
    blocking(move || {
        let docker = docker_status_inner();
        Ok(AppInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            workspace_dir: state.paths.workspace_dir.to_string_lossy().to_string(),
            data_dir: state.paths.data_dir.to_string_lossy().to_string(),
            docker_available: docker.available,
            docker_version: docker.version,
        })
    })
    .await
}

#[tauri::command]
async fn docker_status() -> Result<DockerStatus, AppError> {
    blocking(|| Ok(docker_status_inner())).await
}

#[tauri::command]
async fn add_project(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<ProjectDetail, AppError> {
    let state = state.inner().clone();
    let progress_app = app.clone();
    blocking(move || {
        install_from_url(&state.paths, &state.store, &url, |message| {
            let _ = progress_app.emit(
                "install-progress",
                InstallProgress {
                    message: message.to_string(),
                },
            );
        })
    })
    .await
}

#[tauri::command]
fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, AppError> {
    state.store.list_projects()
}

#[tauri::command]
fn project_detail(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<ProjectDetail, AppError> {
    load_detail(&state.store, &project_id)
}

#[tauri::command]
async fn remove_project(
    state: State<'_, AppState>,
    project_id: String,
    delete_files: bool,
) -> Result<(), AppError> {
    let state = state.inner().clone();
    if state.runs.get(&project_id).is_some() {
        return Err(AppError::new(
            "YOURGIT-3004",
            "Projeto em execução",
            "Pare a execução antes de remover o projeto.",
        ));
    }
    blocking(move || remove_project_files(&state.paths, &state.store, &project_id, delete_files))
        .await
}

#[tauri::command]
fn start_project(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
) -> Result<RunInfo, AppError> {
    let state = state.inner().clone();
    let project = state.store.get_project(&project_id)?;
    let plan = state.store.get_plan(&project_id)?.ok_or_else(|| {
        AppError::new(
            "YOURGIT-4003",
            "Plano não encontrado",
            "Este projeto não tem um plano de execução salvo.",
        )
    })?;

    if plan.start.is_empty() {
        return Err(AppError::new(
            "YOURGIT-3002",
            "Plano sem comando de execução",
            "Não há como iniciar este projeto com o plano atual.",
        )
        .with_hint("Editar planos pela UI entra na próxima etapa; por enquanto use um projeto com script dev/start."));
    }
    if !Path::new(&project.local_path).is_dir() {
        return Err(AppError::new(
            "YOURGIT-3003",
            "Pasta do projeto ausente",
            "A pasta do projeto não existe mais no workspace.",
        ));
    }

    let run_id = Uuid::new_v4().to_string();
    let run_dir = state.paths.logs_dir.join(&project_id);
    std::fs::create_dir_all(&run_dir)?;
    let log_path = run_dir.join(format!("{run_id}.log"));

    let record = RunRecord {
        id: run_id.clone(),
        project_id: project_id.clone(),
        mode: "native".into(),
        state: "starting".into(),
        started_at: Some(now()),
        ended_at: None,
        exit_code: None,
        error_code: None,
        log_path: Some(log_path.to_string_lossy().to_string()),
    };
    state.store.insert_run(&record)?;
    state.store.set_project_state(&project_id, "starting")?;

    let control = RunControl::new(run_id.clone());
    if let Err(error) = state.runs.insert(&project_id, control.clone()) {
        let _ = state
            .store
            .finish_run(&run_id, "error", None, Some(&error.code));
        let _ = state.store.set_project_state(&project_id, "ready");
        return Err(error);
    }

    runner::spawn_run(
        app.clone(),
        state.store.clone(),
        state.runs.clone(),
        project,
        plan,
        control,
        log_path,
    );

    let _ = app.emit(
        "run-status",
        RunStatusEvent {
            project_id: project_id.clone(),
            run_id: run_id.clone(),
            state: "starting".into(),
            message: Some("Preparando execução...".into()),
            exit_code: None,
        },
    );

    Ok(RunInfo {
        run_id,
        project_id,
        state: "starting".into(),
    })
}

#[tauri::command]
fn stop_project(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
) -> Result<(), AppError> {
    let state = state.inner().clone();
    let Some(control) = state.runs.get(&project_id) else {
        return Err(AppError::new(
            "YOURGIT-3005",
            "Projeto não está em execução",
            "Não há uma execução ativa para parar.",
        ));
    };
    state.runs.stop(&project_id);
    runner::mark_stopping(&app, &state.store, &project_id, &control)?;
    Ok(())
}

#[tauri::command]
fn list_runs(state: State<'_, AppState>, project_id: String) -> Result<Vec<RunRecord>, AppError> {
    state.store.list_runs(&project_id, 25)
}

#[tauri::command]
async fn read_log(
    state: State<'_, AppState>,
    project_id: String,
    run_id: String,
) -> Result<Vec<LogLine>, AppError> {
    let state = state.inner().clone();
    blocking(move || {
        let run = state.store.get_run(&run_id)?;
        if run.project_id != project_id {
            return Err(AppError::new(
                "YOURGIT-4003",
                "Log não encontrado",
                "Esta execução não pertence ao projeto informado.",
            ));
        }
        let Some(path) = run.log_path else {
            return Ok(Vec::new());
        };
        let path = PathBuf::from(path);
        if !path.starts_with(&state.paths.logs_dir) {
            return Err(AppError::new(
                "YOURGIT-4005",
                "Caminho de log inválido",
                "O arquivo de log está fora da pasta de logs do YourGit.",
            ));
        }
        runner::read_log_tail(&path)
    })
    .await
}

#[tauri::command]
fn open_url(url: String) -> Result<(), AppError> {
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(AppError::new(
            "YOURGIT-2002",
            "URL inválida",
            "Apenas endereços http(s) podem ser abertos.",
        ));
    }
    open::that(url).map_err(|error| {
        AppError::new(
            "YOURGIT-2002",
            "Não foi possível abrir o navegador",
            error.to_string(),
        )
    })
}

#[tauri::command]
fn open_project_folder(state: State<'_, AppState>, project_id: String) -> Result<(), AppError> {
    let project = state.store.get_project(&project_id)?;
    let path = PathBuf::from(&project.local_path);
    if !path.exists() {
        return Err(AppError::new(
            "YOURGIT-2003",
            "Pasta não encontrada",
            "A pasta do projeto não existe mais.",
        ));
    }
    if !state.paths.is_inside_workspace(&path) {
        return Err(AppError::new(
            "YOURGIT-2003",
            "Pasta fora do workspace",
            "Por segurança, o YourGit só abre pastas dentro do workspace.",
        ));
    }
    open::that(path).map_err(|error| {
        AppError::new(
            "YOURGIT-2003",
            "Não foi possível abrir a pasta",
            error.to_string(),
        )
    })
}

#[tauri::command]
fn reset_dependencies(state: State<'_, AppState>, project_id: String) -> Result<(), AppError> {
    if state.runs.get(&project_id).is_some() {
        return Err(AppError::new(
            "YOURGIT-3004",
            "Projeto em execução",
            "Pare a execução antes de reinstalar as dependências.",
        ));
    }
    state.store.set_deps_ready(&project_id, false)?;
    state.store.set_project_state(&project_id, "ready")?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            app_info,
            docker_status,
            add_project,
            list_projects,
            project_detail,
            remove_project,
            start_project,
            stop_project,
            list_runs,
            read_log,
            open_url,
            open_project_folder,
            reset_dependencies,
        ])
        .setup(|app| {
            let paths = AppPaths::discover().expect("não foi possível descobrir os diretórios");
            paths
                .ensure()
                .expect("não foi possível criar os diretórios");
            let store = Store::open(&paths.db_path).expect("não foi possível abrir o banco");
            store
                .reconcile_orphans()
                .expect("não foi possível reconciliar execuções antigas");
            app.manage(AppState {
                paths,
                store,
                runs: RunRegistry::default(),
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("erro ao construir o YourGit");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            let state = app_handle.state::<AppState>();
            state.runs.stop_all();
        }
    });
}
