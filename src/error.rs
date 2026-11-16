use std::path::PathBuf;
use thiserror::Error;

/// Comprehensive error type for reddiff operations.
#[derive(Error, Debug)]
pub enum ReddiffError {
    #[error("Git error: {0}")]
    Git(String),

    #[error("Diff parsing error: {0}")]
    DiffParse(String),

    #[error("Worktree error at {path}: {message}")]
    Worktree { path: PathBuf, message: String },

    #[error("Runner error: {0}")]
    Runner(String),

    #[error("Execution timed out after {0} seconds")]
    Timeout(u64),

    #[error("Synthesis error: {0}")]
    Synthesis(String),

    #[error("LLM API error: {0}")]
    Llm(String),

    #[error("CLI argument error: {0}")]
    InvalidCli(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
