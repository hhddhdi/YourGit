use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::{Project, RunPlan, RunRecord};
use crate::error::AppError;
use crate::now;

#[derive(Debug, Clone)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = Self {
            path: path.to_path_buf(),
        };
        store.initialize()?;
        Ok(store)
    }

    fn connection(&self) -> Result<Connection, AppError> {
        let connection = Connection::open(&self.path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;",
        )?;
        Ok(connection)
    }

    fn initialize(&self) -> Result<(), AppError> {
        let connection = self.connection()?;
        connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS projects (
                id              TEXT PRIMARY KEY,
                name            TEXT NOT NULL,
                owner           TEXT,
                repo            TEXT,
                source_url      TEXT NOT NULL,
                requested_ref   TEXT,
                resolved_commit TEXT,
                subdir          TEXT NOT NULL DEFAULT '.',
                local_path      TEXT NOT NULL UNIQUE,
                state           TEXT NOT NULL,
                deps_ready      INTEGER NOT NULL DEFAULT 0,
                created_at      TEXT NOT NULL,
                updated_at      TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS run_plans (
                project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
                plan_json  TEXT NOT NULL,
                plan_hash  TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS runs (
                id           TEXT PRIMARY KEY,
                project_id   TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                mode         TEXT NOT NULL,
                state        TEXT NOT NULL,
                started_at   TEXT,
                ended_at     TEXT,
                exit_code    INTEGER,
                error_code   TEXT,
                log_path     TEXT
            );

            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_projects_state ON projects(state);
            CREATE INDEX IF NOT EXISTS idx_runs_project ON runs(project_id, started_at DESC);
            "#,
        )?;
        Ok(())
    }

    pub fn insert_project(&self, project: &Project) -> Result<(), AppError> {
        let connection = self.connection()?;
        connection.execute(
            r#"INSERT INTO projects
               (id, name, owner, repo, source_url, requested_ref, resolved_commit, subdir,
                local_path, state, deps_ready, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
            params![
                project.id,
                project.name,
                project.owner,
                project.repo,
                project.source_url,
                project.requested_ref,
                project.resolved_commit,
                project.subdir,
                project.local_path,
                project.state,
                if project.deps_ready { 1 } else { 0 },
                project.created_at,
                project.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn insert_project_with_plan(
        &self,
        project: &Project,
        plan: &RunPlan,
    ) -> Result<(), AppError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            r#"INSERT INTO projects
               (id, name, owner, repo, source_url, requested_ref, resolved_commit, subdir,
                local_path, state, deps_ready, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
            params![
                project.id,
                project.name,
                project.owner,
                project.repo,
                project.source_url,
                project.requested_ref,
                project.resolved_commit,
                project.subdir,
                project.local_path,
                project.state,
                if project.deps_ready { 1 } else { 0 },
                project.created_at,
                project.updated_at,
            ],
        )?;
        transaction.execute(
            r#"INSERT INTO run_plans (project_id, plan_json, plan_hash, updated_at)
               VALUES (?1, ?2, ?3, ?4)"#,
            params![
                project.id,
                serde_json::to_string(plan)?,
                plan.plan_hash,
                now(),
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn save_plan(&self, project_id: &str, plan: &RunPlan) -> Result<(), AppError> {
        let connection = self.connection()?;
        let timestamp = now();
        connection.execute(
            r#"INSERT INTO run_plans (project_id, plan_json, plan_hash, updated_at)
               VALUES (?1, ?2, ?3, ?4)
               ON CONFLICT(project_id) DO UPDATE SET
                 plan_json = excluded.plan_json,
                 plan_hash = excluded.plan_hash,
                 updated_at = excluded.updated_at"#,
            params![
                project_id,
                serde_json::to_string(plan)?,
                plan.plan_hash,
                timestamp
            ],
        )?;
        Ok(())
    }

    pub fn get_plan(&self, project_id: &str) -> Result<Option<RunPlan>, AppError> {
        let connection = self.connection()?;
        let json: Option<String> = connection
            .query_row(
                "SELECT plan_json FROM run_plans WHERE project_id = ?1",
                params![project_id],
                |row| row.get(0),
            )
            .optional()?;
        match json {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    pub fn get_project(&self, id: &str) -> Result<Project, AppError> {
        let connection = self.connection()?;
        connection
            .query_row(
                r#"SELECT id, name, owner, repo, source_url, requested_ref, resolved_commit,
                          subdir, local_path, state, deps_ready, created_at, updated_at
                   FROM projects WHERE id = ?1"#,
                params![id],
                row_to_project,
            )
            .optional()?
            .ok_or_else(|| {
                AppError::new(
                    "YOURGIT-4002",
                    "Projeto não encontrado",
                    "O projeto pode ter sido removido.",
                )
            })
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, AppError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            r#"SELECT id, name, owner, repo, source_url, requested_ref, resolved_commit,
                      subdir, local_path, state, deps_ready, created_at, updated_at
               FROM projects ORDER BY updated_at DESC"#,
        )?;
        let rows = statement.query_map([], row_to_project)?;
        let mut projects = Vec::new();
        for row in rows {
            projects.push(row?);
        }
        Ok(projects)
    }

    pub fn set_project_state(&self, id: &str, state: &str) -> Result<(), AppError> {
        let connection = self.connection()?;
        connection.execute(
            "UPDATE projects SET state = ?1, updated_at = ?2 WHERE id = ?3",
            params![state, now(), id],
        )?;
        Ok(())
    }

    pub fn set_deps_ready(&self, id: &str, ready: bool) -> Result<(), AppError> {
        let connection = self.connection()?;
        connection.execute(
            "UPDATE projects SET deps_ready = ?1, updated_at = ?2 WHERE id = ?3",
            params![if ready { 1 } else { 0 }, now(), id],
        )?;
        Ok(())
    }

    pub fn delete_project(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connection()?;
        connection.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn insert_run(&self, run: &RunRecord) -> Result<(), AppError> {
        let connection = self.connection()?;
        connection.execute(
            r#"INSERT INTO runs
               (id, project_id, mode, state, started_at, ended_at, exit_code, error_code, log_path)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
            params![
                run.id,
                run.project_id,
                run.mode,
                run.state,
                run.started_at,
                run.ended_at,
                run.exit_code,
                run.error_code,
                run.log_path,
            ],
        )?;
        Ok(())
    }

    pub fn update_run_state(&self, id: &str, state: &str) -> Result<(), AppError> {
        let connection = self.connection()?;
        connection.execute(
            "UPDATE runs SET state = ?1 WHERE id = ?2",
            params![state, id],
        )?;
        Ok(())
    }

    pub fn finish_run(
        &self,
        id: &str,
        state: &str,
        exit_code: Option<i32>,
        error_code: Option<&str>,
    ) -> Result<(), AppError> {
        let connection = self.connection()?;
        connection.execute(
            "UPDATE runs SET state = ?1, ended_at = ?2, exit_code = ?3, error_code = ?4 WHERE id = ?5",
            params![state, now(), exit_code, error_code, id],
        )?;
        Ok(())
    }

    pub fn get_run(&self, id: &str) -> Result<RunRecord, AppError> {
        let connection = self.connection()?;
        connection
            .query_row(
                r#"SELECT id, project_id, mode, state, started_at, ended_at, exit_code,
                          error_code, log_path
                   FROM runs WHERE id = ?1"#,
                params![id],
                row_to_run,
            )
            .optional()?
            .ok_or_else(|| {
                AppError::new(
                    "YOURGIT-4003",
                    "Execução não encontrada",
                    "O histórico desta execução não existe mais.",
                )
            })
    }

    pub fn list_runs(&self, project_id: &str, limit: u32) -> Result<Vec<RunRecord>, AppError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            r#"SELECT id, project_id, mode, state, started_at, ended_at, exit_code,
                      error_code, log_path
               FROM runs WHERE project_id = ?1
               ORDER BY started_at DESC LIMIT ?2"#,
        )?;
        let rows = statement.query_map(params![project_id, limit], row_to_run)?;
        let mut runs = Vec::new();
        for row in rows {
            runs.push(row?);
        }
        Ok(runs)
    }

    pub fn reconcile_orphans(&self) -> Result<(), AppError> {
        let connection = self.connection()?;
        let timestamp = now();
        connection.execute(
            "UPDATE runs SET state = 'interrupted', ended_at = ?1
             WHERE state IN ('starting', 'installing', 'running', 'stopping')",
            params![timestamp],
        )?;
        connection.execute(
            "UPDATE projects SET state = 'stopped', updated_at = ?1
             WHERE state IN ('starting', 'installing', 'running', 'stopping')
                OR id IN (SELECT project_id FROM runs WHERE state = 'interrupted')",
            params![timestamp],
        )?;
        Ok(())
    }
}

fn row_to_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        name: row.get(1)?,
        owner: row.get(2)?,
        repo: row.get(3)?,
        source_url: row.get(4)?,
        requested_ref: row.get(5)?,
        resolved_commit: row.get(6)?,
        subdir: row.get(7)?,
        local_path: row.get(8)?,
        state: row.get(9)?,
        deps_ready: row.get::<_, i64>(10)? != 0,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn row_to_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunRecord> {
    Ok(RunRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        mode: row.get(2)?,
        state: row.get(3)?,
        started_at: row.get(4)?,
        ended_at: row.get(5)?,
        exit_code: row.get(6)?,
        error_code: row.get(7)?,
        log_path: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::StackInfo;

    fn project(id: &str, path: &Path) -> Project {
        Project {
            id: id.into(),
            name: "demo".into(),
            owner: Some("owner".into()),
            repo: Some("demo".into()),
            source_url: "https://github.com/owner/demo".into(),
            requested_ref: None,
            resolved_commit: None,
            subdir: ".".into(),
            local_path: path.to_string_lossy().into(),
            state: "ready".into(),
            deps_ready: false,
            created_at: now(),
            updated_at: now(),
        }
    }

    fn plan() -> RunPlan {
        RunPlan {
            schema_version: 1,
            stack: StackInfo {
                language: "node".into(),
                framework: Some("Vite".into()),
                package_manager: Some("npm".into()),
                confidence: 0.9,
                evidence: vec!["package.json".into()],
            },
            workdir: ".".into(),
            install: vec![vec!["npm".into(), "ci".into()]],
            start: vec![vec!["npm".into(), "run".into(), "dev".into()]],
            ports: vec![],
            env: vec![],
            warnings: vec![],
            plan_hash: "sha256:test".into(),
        }
    }

    #[test]
    fn persists_projects_plans_and_runs() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(&temp.path().join("test.db")).unwrap();
        let project = project("p1", temp.path());
        store.insert_project(&project).unwrap();
        assert_eq!(store.list_projects().unwrap().len(), 1);

        let plan = plan();
        store.save_plan("p1", &plan).unwrap();
        assert_eq!(
            store.get_plan("p1").unwrap().unwrap().stack.language,
            "node"
        );

        let run = RunRecord {
            id: "r1".into(),
            project_id: "p1".into(),
            mode: "native".into(),
            state: "running".into(),
            started_at: Some(now()),
            ended_at: None,
            exit_code: None,
            error_code: None,
            log_path: None,
        };
        store.insert_run(&run).unwrap();
        assert_eq!(store.list_runs("p1", 10).unwrap().len(), 1);

        store.reconcile_orphans().unwrap();
        assert_eq!(store.get_run("r1").unwrap().state, "interrupted");
        assert_eq!(store.get_project("p1").unwrap().state, "stopped");

        store.delete_project("p1").unwrap();
        assert!(store.get_project("p1").is_err());
    }

    #[test]
    fn inserts_project_and_plan_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(&temp.path().join("test.db")).unwrap();
        let project = project("p2", temp.path());
        store.insert_project_with_plan(&project, &plan()).unwrap();
        assert!(store.get_plan("p2").unwrap().is_some());
    }
}
