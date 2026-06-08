//! Error types for nexus_dictation.

use thiserror::Error;

/// Result alias for dictation operations.
pub type Result<T> = std::result::Result<T, DictationError>;

/// Errors that can occur during dictation.
#[derive(Debug, Error)]
pub enum DictationError {
    #[error("audio error: {0}")]
    Audio(String),

    #[error("ASR error: {0}")]
    Asr(String),

    #[error("injection error: {0}")]
    Inject(String),

    #[error("hotkey error: {0}")]
    Hotkey(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("UI error: {0}")]
    Ui(String),

    #[error("pipeline error: {0}")]
    Pipeline(String),

    #[error("format error: {0}")]
    Format(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
