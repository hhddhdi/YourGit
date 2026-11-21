use serde::Serialize;

/// Erro de aplicação com código estável e mensagem amigável.
/// É serializado para o frontend pelo Tauri.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub title: String,
    pub message: String,
    pub details: Option<Box<String>>,
    pub hint: Option<Box<String>>,
    pub retryable: bool,
}

impl AppError {
    pub fn new(code: &str, title: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            title: title.to_string(),
            message: message.into(),
            details: None,
            hint: None,
            retryable: false,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(Box::new(details.into()));
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(Box::new(hint.into()));
        self
    }

    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("YOURGIT-5001", "Erro interno", message).retryable()
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.code, self.title, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::new("YOURGIT-2001", "Falha de arquivo", error.to_string()).retryable()
    }
}

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        Self::new("YOURGIT-1002", "Falha de rede", error.to_string()).retryable()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        Self::new("YOURGIT-4001", "Falha no banco de dados", error.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::new("YOURGIT-5002", "Dados inválidos", error.to_string())
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::new(
            "YOURGIT-1005",
            "Arquivo compactado inválido",
            error.to_string(),
        )
    }
}
