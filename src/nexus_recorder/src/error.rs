//! Error types for the nexus_recorder crate.

use thiserror::Error;

/// Result type alias using `RecorderError`.
pub type Result<T> = std::result::Result<T, RecorderError>;

/// Errors that can occur during recording operations.
#[derive(Debug, Error)]
pub enum RecorderError {
    /// Audio capture or encoding error.
    #[error("Audio error: {0}")]
    Audio(String),

    /// Screen capture error.
    #[error("Screen capture error: {0}")]
    Screen(String),

    /// Video encoding error.
    #[error("Video encoding error: {0}")]
    Video(String),

    /// Input capture error (keyboard/mouse).
    #[error("Input capture error: {0}")]
    Input(String),

    /// Storage/file I/O error.
    #[error("Storage error: {0}")]
    Storage(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Generic error for other cases.
    #[error("{0}")]
    Other(String),
}

impl From<String> for RecorderError {
    fn from(s: String) -> Self {
        RecorderError::Other(s)
    }
}

impl From<&str> for RecorderError {
    fn from(s: &str) -> Self {
        RecorderError::Other(s.to_string())
    }
}

// FFmpeg error conversion
impl From<ffmpeg_next::Error> for RecorderError {
    fn from(err: ffmpeg_next::Error) -> Self {
        RecorderError::Video(err.to_string())
    }
}
