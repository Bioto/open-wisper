//! Recording modules for screen, audio, and input capture.

mod audio;
mod input;
mod postprocess;
mod screen;

pub use audio::{AudioRecorder, AudioConfig, list_devices as list_audio_devices};
pub use input::{InputCapture, InputConfig};
pub use postprocess::{overlay_clicks, process_session_clicks, ClickOverlayConfig};
pub use screen::{ScreenRecorder, ScreenConfig};
