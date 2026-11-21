use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use yourgit_core::clean_log_line;
use yourgit_core::domain::{LogLine, Project, RunPlan};
use yourgit_core::error::AppError;
use yourgit_core::inspect::plan_workdir;
use yourgit_core::store::Store;

const LOG_MAX_BYTES: u64 = 2 * 1024 * 1024;
const LOG_MAX_LINES: usize = 2_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStatusEvent {
    pub project_id: String,
    pub run_id: String,
    pub state: String,
    pub message: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunLogEvent {
    pub project_id: String,
    pub run_id: String,
    pub stream: String,
    pub line: String,
}

#[derive(Clone)]
pub struct RunControl {
    pub run_id: String,
    pub stop: Arc<AtomicBool>,
    pub pid: Arc<Mutex<Option<u32>>>,
}

impl RunControl {
    pub fn new(run_id: String) -> Self {
        Self {
            run_id,
            stop: Arc::new(AtomicBool::new(false)),
            pid: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Clone, Default)]
pub struct RunRegistry {
    inner: Arc<Mutex<HashMap<String, RunControl>>>,
}

impl RunRegistry {
    pub fn insert(&self, project_id: &str, control: RunControl) -> Result<(), AppError> {
        let mut map = self.inner.lock().unwrap();
        if map.contains_key(project_id) {
            return Err(AppError::new(
                "YOURGIT-3004",
                "Projeto já está em execução",
                "Pare a execução atual antes de iniciar outra.",
            ));
        }
        map.insert(project_id.to_string(), control);
        Ok(())
    }

    pub fn get(&self, project_id: &str) -> Option<RunControl> {
        self.inner.lock().unwrap().get(project_id).cloned()
    }

    pub fn remove(&self, project_id: &str) {
        self.inner.lock().unwrap().remove(project_id);
    }

    pub fn stop(&self, project_id: &str) -> bool {
        let control = self.get(project_id);
        let Some(control) = control else {
            return false;
        };
        request_stop(&control);
        true
    }

    pub fn stop_all(&self) {
        let controls = self
            .inner
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for control in controls {
            request_stop(&control);
        }
    }
}

fn request_stop(control: &RunControl) {
    control.stop.store(true, Ordering::SeqCst);
    let pid = *control.pid.lock().unwrap();
    if let Some(pid) = pid {
        kill_tree(pid);
    }
}

pub fn spawn_run(
    app: AppHandle,
    store: Store,
    registry: RunRegistry,
    project: Project,
    plan: RunPlan,
    control: RunControl,
    log_path: PathBuf,
) {
    let project_id = project.id.clone();
    thread::spawn(move || {
        if let Err(error) = execute_run(&app, &store, &project, &plan, &control, &log_path) {
            let _ = store.finish_run(&control.run_id, "error", None, Some(&error.code));
            let _ = store.set_project_state(&project.id, "error");
            let _ = app.emit(
                "run-status",
                RunStatusEvent {
                    project_id: project.id.clone(),
                    run_id: control.run_id.clone(),
                    state: "error".into(),
                    message: Some(format!("{}: {}", error.title, error.message)),
                    exit_code: None,
                },
            );
        }
        registry.remove(&project_id);
    });
}

fn execute_run(
    app: &AppHandle,
    store: &Store,
    project: &Project,
    plan: &RunPlan,
    control: &RunControl,
    log_path: &Path,
) -> Result<(), AppError> {
    let cwd = plan_workdir(Path::new(&project.local_path), plan)?;
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let writer = Arc::new(Mutex::new(log_file));
    let run_id = control.run_id.clone();
    let project_id = project.id.clone();

    let ctx = RunContext {
        app,
        store,
        writer: &writer,
        project_id: project_id.as_str(),
        run_id: run_id.as_str(),
        control,
        cwd: cwd.as_path(),
    };

    system_log(
        app,
        &writer,
        &project_id,
        &run_id,
        &format!("Iniciando execução nativa em {}", cwd.display()),
    );

    if !project.deps_ready && !plan.install.is_empty() {
        update_state(
            app,
            store,
            &project_id,
            &run_id,
            "installing",
            Some("Instalando dependências..."),
        )?;

        for command in &plan.install {
            if control.stop.load(Ordering::SeqCst) {
                finish_stopped(app, store, &project_id, &run_id, None, &writer)?;
                return Ok(());
            }

            system_log(
                app,
                &writer,
                &project_id,
                &run_id,
                &format!("$ {}", display_command(command)),
            );

            let status = spawn_and_wait(&ctx, command, false)?;

            if control.stop.load(Ordering::SeqCst) {
                finish_stopped(app, store, &project_id, &run_id, status.code(), &writer)?;
                return Ok(());
            }

            if !status.success() {
                return Err(AppError::new(
                    "YOURGIT-3001",
                    "Instalação falhou",
                    format!(
                        "O comando `{}` terminou com código {:?}.",
                        display_command(command),
                        status.code()
                    ),
                )
                .with_hint("Veja os logs acima e ajuste as dependências ou o plano."));
            }
        }

        store.set_deps_ready(&project_id, true)?;
        system_log(
            app,
            &writer,
            &project_id,
            &run_id,
            "Dependências instaladas.",
        );
    }

    if control.stop.load(Ordering::SeqCst) {
        finish_stopped(app, store, &project_id, &run_id, None, &writer)?;
        return Ok(());
    }

    let Some(start) = plan.start.first() else {
        return Err(AppError::new(
            "YOURGIT-3002",
            "Comando de execução ausente",
            "O plano não define como iniciar este projeto.",
        )
        .with_hint("Edite o plano e informe um comando de start."));
    };

    update_state(
        app,
        store,
        &project_id,
        &run_id,
        "starting",
        Some("Iniciando aplicação..."),
    )?;

    system_log(
        app,
        &writer,
        &project_id,
        &run_id,
        &format!("$ {}", display_command(start)),
    );

    let status = spawn_and_wait(&ctx, start, true)?;

    if control.stop.load(Ordering::SeqCst) {
        finish_stopped(app, store, &project_id, &run_id, status.code(), &writer)?;
    } else if status.success() {
        finish_state(
            app,
            store,
            &project_id,
            &run_id,
            "stopped",
            status.code(),
            Some("A aplicação terminou."),
        )?;
    } else {
        finish_state(
            app,
            store,
            &project_id,
            &run_id,
            "error",
            status.code(),
            Some("A aplicação terminou com erro."),
        )?;
    }

    Ok(())
}

struct RunContext<'a> {
    app: &'a AppHandle,
    store: &'a Store,
    writer: &'a Arc<Mutex<File>>,
    project_id: &'a str,
    run_id: &'a str,
    control: &'a RunControl,
    cwd: &'a Path,
}

fn spawn_and_wait(
    ctx: &RunContext<'_>,
    command: &[String],
    mark_running: bool,
) -> Result<std::process::ExitStatus, AppError> {
    let Some(program) = command.first() else {
        return Err(AppError::new(
            "YOURGIT-3003",
            "Comando inválido",
            "O plano contém um comando vazio.",
        ));
    };
    let args = &command[1..];

    let mut child_command = command_for(program, args);
    child_command
        .current_dir(ctx.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("YOURGIT", "1")
        .env("NO_COLOR", "1")
        .env("CLICOLOR", "0")
        .env("npm_config_color", "false");
    hide_window(&mut child_command);

    let mut child = child_command.spawn().map_err(|error| {
        AppError::new(
            "YOURGIT-3003",
            "Não foi possível iniciar o processo",
            format!("`{program}` falhou ao iniciar: {error}"),
        )
        .with_hint("Confira se a ferramenta está instalada e disponível no PATH.")
    })?;

    let pid = child.id();
    *ctx.control.pid.lock().unwrap() = Some(pid);
    if ctx.control.stop.load(Ordering::SeqCst) {
        kill_tree(pid);
    }

    if mark_running {
        let _ = update_state(
            ctx.app,
            ctx.store,
            ctx.project_id,
            ctx.run_id,
            "running",
            Some("Aplicação em execução."),
        );
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let out_handle = stdout.map(|stream| {
        spawn_reader(
            stream,
            "stdout",
            ctx.app.clone(),
            ctx.project_id.to_string(),
            ctx.run_id.to_string(),
            ctx.writer.clone(),
        )
    });
    let err_handle = stderr.map(|stream| {
        spawn_reader(
            stream,
            "stderr",
            ctx.app.clone(),
            ctx.project_id.to_string(),
            ctx.run_id.to_string(),
            ctx.writer.clone(),
        )
    });

    let status = child.wait()?;
    if let Some(handle) = out_handle {
        let _ = handle.join();
    }
    if let Some(handle) = err_handle {
        let _ = handle.join();
    }
    *ctx.control.pid.lock().unwrap() = None;
    Ok(status)
}

fn spawn_reader<R: Read + Send + 'static>(
    reader: R,
    stream: &'static str,
    app: AppHandle,
    project_id: String,
    run_id: String,
    writer: Arc<Mutex<File>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) => break,
                Ok(_) => {
                    let raw = String::from_utf8_lossy(&buffer);
                    let cleaned = clean_log_line(raw.trim_end_matches(['\r', '\n']));
                    let line = truncate(cleaned, 16_000);
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(mut file) = writer.lock() {
                        let _ = writeln!(file, "[{stream}] {line}");
                        let _ = file.flush();
                    }
                    let _ = app.emit(
                        "run-log",
                        RunLogEvent {
                            project_id: project_id.clone(),
                            run_id: run_id.clone(),
                            stream: stream.to_string(),
                            line,
                        },
                    );
                }
                Err(_) => break,
            }
        }
    })
}

fn update_state(
    app: &AppHandle,
    store: &Store,
    project_id: &str,
    run_id: &str,
    state: &str,
    message: Option<&str>,
) -> Result<(), AppError> {
    store.set_project_state(project_id, state)?;
    store.update_run_state(run_id, state)?;
    let _ = app.emit(
        "run-status",
        RunStatusEvent {
            project_id: project_id.to_string(),
            run_id: run_id.to_string(),
            state: state.to_string(),
            message: message.map(str::to_string),
            exit_code: None,
        },
    );
    Ok(())
}

fn finish_stopped(
    app: &AppHandle,
    store: &Store,
    project_id: &str,
    run_id: &str,
    exit_code: Option<i32>,
    writer: &Arc<Mutex<File>>,
) -> Result<(), AppError> {
    system_log(
        app,
        writer,
        project_id,
        run_id,
        "Execução interrompida pelo usuário.",
    );
    finish_state(
        app,
        store,
        project_id,
        run_id,
        "stopped",
        exit_code,
        Some("Execução parada."),
    )
}

fn finish_state(
    app: &AppHandle,
    store: &Store,
    project_id: &str,
    run_id: &str,
    state: &str,
    exit_code: Option<i32>,
    message: Option<&str>,
) -> Result<(), AppError> {
    store.finish_run(run_id, state, exit_code, None)?;
    store.set_project_state(project_id, state)?;
    let _ = app.emit(
        "run-status",
        RunStatusEvent {
            project_id: project_id.to_string(),
            run_id: run_id.to_string(),
            state: state.to_string(),
            message: message.map(str::to_string),
            exit_code,
        },
    );
    Ok(())
}

fn system_log(
    app: &AppHandle,
    writer: &Arc<Mutex<File>>,
    project_id: &str,
    run_id: &str,
    line: &str,
) {
    let line = truncate(clean_log_line(line), 16_000);
    if let Ok(mut file) = writer.lock() {
        let _ = writeln!(file, "[system] {line}");
        let _ = file.flush();
    }
    let _ = app.emit(
        "run-log",
        RunLogEvent {
            project_id: project_id.to_string(),
            run_id: run_id.to_string(),
            stream: "system".into(),
            line,
        },
    );
}

pub fn read_log_tail(path: &Path) -> Result<Vec<LogLine>, AppError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    if length > LOG_MAX_BYTES {
        file.seek(SeekFrom::Start(length - LOG_MAX_BYTES))?;
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = Vec::new();
    for raw in text.lines() {
        let Some(rest) = raw.strip_prefix('[') else {
            continue;
        };
        let Some((stream, line)) = rest.split_once("] ") else {
            continue;
        };
        if !matches!(stream, "stdout" | "stderr" | "system") {
            continue;
        }
        lines.push(LogLine {
            stream: stream.to_string(),
            line: line.to_string(),
        });
    }
    if lines.len() > LOG_MAX_LINES {
        lines = lines.split_off(lines.len() - LOG_MAX_LINES);
    }
    Ok(lines)
}

pub fn display_command(command: &[String]) -> String {
    command
        .iter()
        .map(|part| {
            if part.contains(' ') {
                format!("\"{part}\"")
            } else {
                part.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn command_for(program: &str, args: &[String]) -> Command {
    #[cfg(windows)]
    {
        if matches!(program, "npm" | "npx" | "pnpm" | "yarn" | "bun") {
            let mut command = Command::new("cmd");
            command.arg("/C").arg(program).args(args);
            return command;
        }
    }

    let mut command = Command::new(program);
    command.args(args);
    command
}

fn truncate(mut value: String, max: usize) -> String {
    if value.len() <= max {
        return value;
    }
    let mut end = max;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value.push('…');
    value
}

#[cfg(windows)]
pub(crate) fn hide_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
pub(crate) fn hide_window(_command: &mut Command) {}

#[cfg(windows)]
fn kill_tree(pid: u32) {
    let mut command = Command::new("taskkill");
    command.args(["/PID", &pid.to_string(), "/T", "/F"]);
    hide_window(&mut command);
    let _ = command.output();
}

#[cfg(not(windows))]
fn kill_tree(pid: u32) {
    let _ = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output();
}

pub fn mark_stopping(
    app: &AppHandle,
    store: &Store,
    project_id: &str,
    control: &RunControl,
) -> Result<(), AppError> {
    store.set_project_state(project_id, "stopping")?;
    store.update_run_state(&control.run_id, "stopping")?;
    let _ = app.emit(
        "run-status",
        RunStatusEvent {
            project_id: project_id.to_string(),
            run_id: control.run_id.clone(),
            state: "stopping".into(),
            message: Some("Parando execução...".into()),
            exit_code: None,
        },
    );
    Ok(())
}
