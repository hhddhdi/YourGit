use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub owner: Option<String>,
    pub repo: Option<String>,
    pub source_url: String,
    pub requested_ref: Option<String>,
    pub resolved_commit: Option<String>,
    pub subdir: String,
    pub local_path: String,
    /// ready | installing | starting | running | stopping | stopped | error
    pub state: String,
    pub deps_ready: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackInfo {
    pub language: String,
    pub framework: Option<String>,
    pub package_manager: Option<String>,
    pub confidence: f32,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortSpec {
    pub container: u16,
    pub protocol: String,
    pub health_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvSpec {
    pub key: String,
    pub required: bool,
    pub secret: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPlan {
    pub schema_version: u32,
    pub stack: StackInfo,
    pub workdir: String,
    pub install: Vec<Vec<String>>,
    pub start: Vec<Vec<String>>,
    pub ports: Vec<PortSpec>,
    pub env: Vec<EnvSpec>,
    pub warnings: Vec<String>,
    pub plan_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub id: String,
    pub project_id: String,
    pub mode: String,
    pub state: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub exit_code: Option<i32>,
    pub error_code: Option<String>,
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub project: Project,
    pub plan: Option<RunPlan>,
    pub runs: Vec<RunRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunInfo {
    pub run_id: String,
    pub project_id: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub stream: String,
    pub line: String,
}
