//! Nexus Recorder - Desktop recording library with PyO3 bindings.
//!
//! Provides screen, audio, and input capture functionality for desktop recording.
//! Can be used from Rust directly or from Python via PyO3 bindings.
//!
//! # Features
//!
//! - **Screen Recording**: Capture screen to MP4 using FFmpeg
//! - **Audio Recording**: Capture audio to MP4 using FFmpeg  
//! - **Input Capture**: Record keyboard and mouse events to JSONL
//! - **Python Bindings**: Full PyO3 integration for Python scripts
//!
//! # Example (Rust)
//!
//! ```no_run
//! use nexus_recorder::recording::{ScreenRecorder, ScreenConfig};
//!
//! let config = ScreenConfig::default();
//! let recorder = ScreenRecorder::start("output.mp4", config).unwrap();
//! // ... do work ...
//! recorder.stop().unwrap();
//! ```
//!
//! # Example (Python)
//!
//! ```python
//! import nexus_recorder
//!
//! with nexus_recorder.RecordingSession("./output") as session:
//!     session.record_screen(fps=30)
//!     session.record_audio()
//!     session.capture_input()
//!     # ... do work ...
//! # Automatically stops on exit
//! ```

pub mod cli;
pub mod error;
#[cfg(feature = "python")]
pub mod python;
pub mod recording;
pub mod storage;

// Re-export main types
pub use error::{RecorderError, Result};
pub use recording::{
    AudioConfig, AudioRecorder, InputCapture, InputConfig, ScreenConfig, ScreenRecorder,
};
pub use storage::{Event, EventType, EventWriter, MouseAction};

// PyO3 module definition (only when building Python extension)
#[cfg(feature = "python")]
use pyo3::prelude::*;

/// Python module for nexus_recorder.
#[cfg(feature = "python")]
#[pymodule]
fn nexus_recorder(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Initialize tracing for the library
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    // Register Python classes
    python::register_module(m)?;

    // Add module-level docstring
    m.add("__doc__", "Desktop recording library for screen, audio, and input capture")?;

    Ok(())
}
