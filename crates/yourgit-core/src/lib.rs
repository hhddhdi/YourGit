pub mod archive;
pub mod domain;
pub mod download;
pub mod error;
pub mod inspect;
pub mod installer;
pub mod paths;
pub mod source;
pub mod store;

use std::sync::LazyLock;

use chrono::{SecondsFormat, Utc};
use regex::Regex;

pub fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

static ANSI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\x1b(?:\[[0-9;?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1b\\))").unwrap()
});

static TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16})",
    )
    .unwrap()
});

static SECRET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(token|secret|password|api[_-]?key)\b\s*[:=]\s*["']?([^\s"']{6,})"#)
        .unwrap()
});

/// Remove códigos ANSI e segredos conhecidos antes de gravar ou enviar um log.
pub fn clean_log_line(line: &str) -> String {
    let without_ansi = ANSI_RE.replace_all(line, "");
    redact_line(&without_ansi)
}

pub fn redact_line(line: &str) -> String {
    let tokens_removed = TOKEN_RE.replace_all(line, "[REDACTED]");
    SECRET_RE
        .replace_all(&tokens_removed, "$1=[REDACTED]")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_github_tokens() {
        let line = "token: ghp_abcdefghijklmnopqrstuvwxyz123456";
        assert!(!redact_line(line).contains("ghp_"));
    }

    #[test]
    fn redacts_key_value_secrets() {
        let line = "API_KEY=super-secret-value";
        assert_eq!(redact_line(line), "API_KEY=[REDACTED]");
    }

    #[test]
    fn strips_ansi_codes() {
        assert_eq!(clean_log_line("\u{1b}[32mhello\u{1b}[0m"), "hello");
    }
}
