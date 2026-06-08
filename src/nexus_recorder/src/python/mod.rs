//! PyO3 Python bindings for nexus_recorder.
//!
//! Exposes screen, audio, and input recording functionality to Python.

use pyo3::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use std::path::PathBuf;

use crate::recording::{
    AudioConfig, AudioRecorder, ClickOverlayConfig, InputCapture, InputConfig, ScreenConfig,
    ScreenRecorder, overlay_clicks as rust_overlay_clicks,
};
use crate::storage::EventWriter;

/// Python wrapper for ScreenRecorder.
#[pyclass]
pub struct PyScreenRecorder {
    inner: Option<ScreenRecorder>,
}

#[pymethods]
impl PyScreenRecorder {
    #[new]
    fn new() -> Self {
        Self { inner: None }
    }

    /// Start screen recording.
    ///
    /// Args:
    ///     output_path: Path to output MP4 file
    ///     fps: Frames per second (default: 30)
    ///     monitor: Monitor index (default: 0)
    #[pyo3(signature = (output_path, fps=30, monitor=0))]
    fn start(&mut self, output_path: &str, fps: u32, monitor: usize) -> PyResult<()> {
        if self.inner.is_some() {
            return Err(PyRuntimeError::new_err("Recording already in progress"));
        }

        let config = ScreenConfig {
            fps,
            monitor,
            ..Default::default()
        };

        let recorder = ScreenRecorder::start(output_path, config)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        self.inner = Some(recorder);
        Ok(())
    }

    /// Stop screen recording and return the output path.
    fn stop(&mut self) -> PyResult<String> {
        let recorder = self
            .inner
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("No recording in progress"))?;

        let path = recorder
            .stop()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(path.to_string_lossy().to_string())
    }

    /// Check if recording is in progress.
    fn is_running(&self) -> bool {
        self.inner.as_ref().map(|r| r.is_running()).unwrap_or(false)
    }
}

/// Python wrapper for AudioRecorder.
#[pyclass]
pub struct PyAudioRecorder {
    inner: Option<AudioRecorder>,
}

#[pymethods]
impl PyAudioRecorder {
    #[new]
    fn new() -> Self {
        Self { inner: None }
    }

    /// Start audio recording.
    ///
    /// Args:
    ///     output_path: Path to output file (MP4/AAC)
    ///     device: Audio device name (default: system default)
    ///     sample_rate: Sample rate in Hz (default: 44100)
    ///     channels: Number of channels (default: 2)
    #[pyo3(signature = (output_path, device=None, sample_rate=44100, channels=2))]
    fn start(
        &mut self,
        output_path: &str,
        device: Option<&str>,
        sample_rate: u32,
        channels: u16,
    ) -> PyResult<()> {
        if self.inner.is_some() {
            return Err(PyRuntimeError::new_err("Recording already in progress"));
        }

        let config = AudioConfig {
            device: device.map(String::from),
            sample_rate,
            channels,
            ..Default::default()
        };

        let recorder = AudioRecorder::start(output_path, config)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        self.inner = Some(recorder);
        Ok(())
    }

    /// Stop audio recording and return the output path.
    fn stop(&mut self) -> PyResult<String> {
        let recorder = self
            .inner
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("No recording in progress"))?;

        let path = recorder
            .stop()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(path.to_string_lossy().to_string())
    }

    /// Check if recording is in progress.
    fn is_running(&self) -> bool {
        self.inner.as_ref().map(|r| r.is_running()).unwrap_or(false)
    }
}

/// Python wrapper for ClickOverlayConfig.
#[pyclass]
#[derive(Clone)]
pub struct PyClickOverlayConfig {
    inner: ClickOverlayConfig,
}

#[pymethods]
impl PyClickOverlayConfig {
    #[new]
    #[pyo3(signature = (click_duration=0.4, indicator_size=24, border_thickness=4, show_ripple=true, ripple_size=48))]
    fn new(
        click_duration: f64,
        indicator_size: u32,
        border_thickness: u32,
        show_ripple: bool,
        ripple_size: u32,
    ) -> Self {
        Self {
            inner: ClickOverlayConfig {
                click_duration,
                indicator_size,
                border_thickness,
                show_ripple,
                ripple_size,
            },
        }
    }

    /// Duration to show each click indicator (seconds).
    #[getter]
    fn click_duration(&self) -> f64 {
        self.inner.click_duration
    }

    /// Size of the click indicator.
    #[getter]
    fn indicator_size(&self) -> u32 {
        self.inner.indicator_size
    }

    /// Border thickness.
    #[getter]
    fn border_thickness(&self) -> u32 {
        self.inner.border_thickness
    }

    /// Whether to show a ripple effect.
    #[getter]
    fn show_ripple(&self) -> bool {
        self.inner.show_ripple
    }

    /// Ripple max size.
    #[getter]
    fn ripple_size(&self) -> u32 {
        self.inner.ripple_size
    }
}

/// Python wrapper for InputCapture.
#[pyclass]
pub struct PyInputCapture {
    inner: Option<InputCapture>,
}

#[pymethods]
impl PyInputCapture {
    #[new]
    fn new() -> Self {
        Self { inner: None }
    }

    /// Start input capture (keyboard + mouse).
    ///
    /// Args:
    ///     output_path: Path to output JSONL file
    ///     capture_keyboard: Capture keyboard events (default: True)
    ///     capture_mouse_clicks: Capture mouse clicks (default: True)
    ///     capture_mouse_moves: Capture mouse movement (default: False)
    #[pyo3(signature = (output_path, capture_keyboard=true, capture_mouse_clicks=true, capture_mouse_moves=false))]
    fn start(
        &mut self,
        output_path: &str,
        capture_keyboard: bool,
        capture_mouse_clicks: bool,
        capture_mouse_moves: bool,
    ) -> PyResult<()> {
        if self.inner.is_some() {
            return Err(PyRuntimeError::new_err("Capture already in progress"));
        }

        let config = InputConfig {
            capture_keyboard,
            capture_mouse_clicks,
            capture_mouse_moves,
            ..Default::default()
        };

        let capture = InputCapture::start(output_path, config)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        self.inner = Some(capture);
        Ok(())
    }

    /// Stop input capture.
    fn stop(&mut self) -> PyResult<()> {
        let capture = self
            .inner
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("No capture in progress"))?;

        capture
            .stop()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(())
    }

    /// Check if capture is in progress.
    fn is_running(&self) -> bool {
        self.inner.as_ref().map(|c| c.is_running()).unwrap_or(false)
    }
}

/// Recording session that manages multiple recorders.
#[pyclass]
pub struct PyRecordingSession {
    output_dir: PathBuf,
    screen: Option<ScreenRecorder>,
    audio: Option<AudioRecorder>,
    input: Option<InputCapture>,
    #[allow(dead_code)]
    event_writer: Option<EventWriter>,
}

#[pymethods]
impl PyRecordingSession {
    /// Create a new recording session.
    ///
    /// Args:
    ///     output_dir: Directory for output files
    #[new]
    fn new(output_dir: &str) -> PyResult<Self> {
        let path = PathBuf::from(output_dir);
        
        // Create output directory if it doesn't exist
        std::fs::create_dir_all(&path)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create output dir: {}", e)))?;

        Ok(Self {
            output_dir: path,
            screen: None,
            audio: None,
            input: None,
            event_writer: None,
        })
    }

    /// Start screen recording.
    #[pyo3(signature = (fps=30, monitor=0))]
    fn record_screen(&mut self, fps: u32, monitor: usize) -> PyResult<()> {
        if self.screen.is_some() {
            return Err(PyRuntimeError::new_err("Screen recording already active"));
        }

        let output_path = self.output_dir.join("screen.mp4");
        let config = ScreenConfig {
            fps,
            monitor,
            ..Default::default()
        };

        let recorder = ScreenRecorder::start(&output_path, config)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        self.screen = Some(recorder);
        Ok(())
    }

    /// Start audio recording.
    #[pyo3(signature = (device=None))]
    fn record_audio(&mut self, device: Option<&str>) -> PyResult<()> {
        if self.audio.is_some() {
            return Err(PyRuntimeError::new_err("Audio recording already active"));
        }

        let output_path = self.output_dir.join("audio.mp4");
        let config = AudioConfig {
            device: device.map(String::from),
            ..Default::default()
        };

        let recorder = AudioRecorder::start(&output_path, config)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        self.audio = Some(recorder);
        Ok(())
    }

    /// Start input capture (keyboard + mouse).
    #[pyo3(signature = (capture_moves=false))]
    fn capture_input(&mut self, capture_moves: bool) -> PyResult<()> {
        if self.input.is_some() {
            return Err(PyRuntimeError::new_err("Input capture already active"));
        }

        let output_path = self.output_dir.join("events.jsonl");
        let config = InputConfig {
            capture_mouse_moves: capture_moves,
            ..Default::default()
        };

        let capture = InputCapture::start(&output_path, config)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        self.input = Some(capture);
        Ok(())
    }

    /// Stop all active recordings.
    fn stop(&mut self) -> PyResult<()> {
        // #region agent log - H3: Track PyRecordingSession stop sequence
        tracing::info!("PyRecordingSession::stop() called, screen={}, audio={}, input={}", 
            self.screen.is_some(), self.audio.is_some(), self.input.is_some());
        // #endregion
        
        if let Some(screen) = self.screen.take() {
            // #region agent log - H3: Stopping screen
            tracing::info!("Stopping screen recorder...");
            // #endregion
            screen
                .stop()
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            // #region agent log - H3: Screen stopped
            tracing::info!("Screen recorder stopped successfully");
            // #endregion
        }

        if let Some(audio) = self.audio.take() {
            // #region agent log - H3: Stopping audio
            tracing::info!("Stopping audio recorder...");
            // #endregion
            audio
                .stop()
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            // #region agent log - H3: Audio stopped
            tracing::info!("Audio recorder stopped successfully");
            // #endregion
        }

        if let Some(input) = self.input.take() {
            // #region agent log - H3: Stopping input
            tracing::info!("Stopping input capture...");
            // #endregion
            input
                .stop()
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            // #region agent log - H3: Input stopped
            tracing::info!("Input capture stopped successfully");
            // #endregion
        }

        // #region agent log - H3: All stopped
        tracing::info!("PyRecordingSession::stop() completed");
        // #endregion
        Ok(())
    }

    /// Get the output directory.
    fn output_dir(&self) -> String {
        self.output_dir.to_string_lossy().to_string()
    }

    /// Overlay click indicators on the recorded video.
    ///
    /// This processes the screen recording and events file to create a new video
    /// with click overlays. Requires both screen recording and input capture to have been active.
    ///
    /// Args:
    ///     output_path: Optional path for output video (default: screen_with_clicks.mp4 in output_dir)
    ///     config: Optional ClickOverlayConfig (default: uses default settings)
    ///
    /// Returns:
    ///     Path to the output video with overlays
    #[pyo3(signature = (output_path=None, config=None))]
    fn overlay_clicks(
        &self,
        output_path: Option<&str>,
        config: Option<PyClickOverlayConfig>,
    ) -> PyResult<String> {
        let video_path = self.output_dir.join("screen.mp4");
        let events_path = self.output_dir.join("events.jsonl");
        let output_path = if let Some(path) = output_path {
            PathBuf::from(path)
        } else {
            self.output_dir.join("screen_with_clicks.mp4")
        };

        if !video_path.exists() {
            return Err(PyRuntimeError::new_err(format!(
                "Screen recording not found: {:?}",
                video_path
            )));
        }

        if !events_path.exists() {
            return Err(PyRuntimeError::new_err(format!(
                "Events file not found: {:?}",
                events_path
            )));
        }

        let overlay_config = config
            .map(|c| c.inner)
            .unwrap_or_else(ClickOverlayConfig::default);

        rust_overlay_clicks(&video_path, &events_path, &output_path, &overlay_config)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(output_path.to_string_lossy().to_string())
    }

    /// Context manager enter.
    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    /// Context manager exit - stops all recordings.
    fn __exit__(
        &mut self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_value: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        self.stop()?;
        Ok(false) // Don't suppress exceptions
    }
}

/// Overlay click indicators on a video.
///
/// Args:
///     video_path: Path to the input video file
///     events_path: Path to the JSONL events file
///     output_path: Path for the output video with overlays
///     config: Optional ClickOverlayConfig (default: uses default settings)
#[pyfunction]
#[pyo3(signature = (video_path, events_path, output_path, config=None))]
fn overlay_clicks(
    video_path: &str,
    events_path: &str,
    output_path: &str,
    config: Option<PyClickOverlayConfig>,
) -> PyResult<()> {
    let overlay_config = config
        .map(|c| c.inner)
        .unwrap_or_else(ClickOverlayConfig::default);

    rust_overlay_clicks(video_path, events_path, output_path, &overlay_config)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

    Ok(())
}

/// Register Python module.
pub fn register_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyScreenRecorder>()?;
    m.add_class::<PyAudioRecorder>()?;
    m.add_class::<PyInputCapture>()?;
    m.add_class::<PyRecordingSession>()?;
    m.add_class::<PyClickOverlayConfig>()?;
    m.add_function(wrap_pyfunction!(overlay_clicks, m)?)?;
    Ok(())
}
