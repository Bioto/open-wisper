//! Audio recording to MP4/AAC.
//!
//! Uses FFmpeg for audio capture and encoding.

use crate::error::{RecorderError, Result};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};


// Unix signal handling for graceful FFmpeg shutdown
#[cfg(unix)]
use nix::sys::signal::{kill, Signal};
#[cfg(unix)]
use nix::unistd::Pid;

/// Wait for a process to exit with a timeout.
/// Returns Some(status) if the process exited, None if timeout expired.
fn wait_with_timeout(process: &mut Child, timeout: Duration) -> Option<std::process::ExitStatus> {
    let start = Instant::now();
    loop {
        match process.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    }
}

/// List available audio devices on the system.
#[cfg(target_os = "linux")]
pub fn list_devices() -> Result<Vec<String>> {
    let output = Command::new("ffmpeg")
        .args(["-sources", "alsa"])
        .output()
        .map_err(|e| RecorderError::Audio(format!("Failed to run ffmpeg: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    let mut devices = Vec::new();
    for line in combined.lines() {
        // Lines with device names start with spaces and contain device info
        let trimmed = line.trim();
        if trimmed.starts_with("sysdefault:") || trimmed.starts_with("front:") || trimmed.starts_with("hw:") {
            if let Some(device_name) = trimmed.split_whitespace().next() {
                devices.push(device_name.to_string());
            }
        }
    }

    Ok(devices)
}

#[cfg(not(target_os = "linux"))]
pub fn list_devices() -> Result<Vec<String>> {
    Ok(vec!["default".to_string()])
}

/// Configuration for audio recording.
#[derive(Debug, Clone)]
pub struct AudioConfig {
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo).
    pub channels: u16,
    /// Audio codec (default: aac).
    pub codec: String,
    /// Audio bitrate (e.g., "128k").
    pub bitrate: String,
    /// Device name (None = default device).
    pub device: Option<String>,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            channels: 2,
            codec: "aac".to_string(),
            bitrate: "128k".to_string(),
            device: None,
        }
    }
}

/// Audio recorder handle.
pub struct AudioRecorder {
    stop_flag: Arc<AtomicBool>,
    output_path: PathBuf,
    ffmpeg_process: Option<Child>,
}

impl AudioRecorder {
    /// Start recording audio to the given output file.
    ///
    /// Uses FFmpeg's pulse (Linux) or avfoundation (macOS) for capture.
    pub fn start<P: AsRef<Path>>(output_path: P, config: AudioConfig) -> Result<Self> {
        let output_path = output_path.as_ref().to_path_buf();
        let stop_flag = Arc::new(AtomicBool::new(false));

        let mut cmd = build_ffmpeg_command(&output_path, &config)?;

        tracing::info!("Starting audio recording to {:?}", output_path);
        tracing::debug!("FFmpeg command: {:?}", cmd);

        // NOTE: We do NOT put FFmpeg in a separate process group.
        // This allows FFmpeg to receive SIGINT directly from the terminal when Ctrl+C is pressed,
        // which triggers FFmpeg's graceful shutdown before we try to stop it.

        let ffmpeg_process = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| RecorderError::Audio(format!("Failed to start FFmpeg: {}", e)))?;

        Ok(Self {
            stop_flag,
            output_path,
            ffmpeg_process: Some(ffmpeg_process),
        })
    }

    /// Stop the audio recording.
    pub fn stop(mut self) -> Result<PathBuf> {
        self.stop_flag.store(true, Ordering::SeqCst);
        // #region agent log - H3: Track audio stop sequence
        tracing::info!("AudioRecorder::stop() called, output_path={:?}", self.output_path);
        // #endregion

        if let Some(mut process) = self.ffmpeg_process.take() {
            let pid = process.id();
            tracing::info!("FFmpeg audio process found, pid={}", pid);
            
            // Send SIGINT to FFmpeg for graceful shutdown (same as Ctrl+C).
            // SIGINT is what FFmpeg expects for interactive quit - it will flush buffers 
            // and write the moov atom.
            #[cfg(unix)]
            {
                let nix_pid = Pid::from_raw(pid as i32);
                match kill(nix_pid, Signal::SIGINT) {
                    Ok(()) => tracing::info!("Sent SIGINT to FFmpeg audio process"),
                    Err(e) => tracing::warn!("Failed to send SIGINT to FFmpeg audio: {}", e),
                }
            }
            
            // On Windows, use stdin 'q' command (FFmpeg reads from pipe on Windows)
            #[cfg(windows)]
            {
                if let Some(mut stdin) = process.stdin.take() {
                    use std::io::Write;
                    let _ = stdin.write_all(b"q");
                    let _ = stdin.flush();
                }
            }

            // Wait for process to finish with timeout, then escalate to kill if needed
            tracing::info!("Waiting for FFmpeg audio to exit (timeout=5s)...");
            if let Some(status) = wait_with_timeout(&mut process, Duration::from_secs(5)) {
                tracing::info!("FFmpeg audio exited with status: {}", status);
                if !status.success() {
                    tracing::warn!("FFmpeg audio exited with non-zero status: {}", status);
                }
            } else {
                // Timeout - FFmpeg didn't respond, force kill
                tracing::warn!("FFmpeg audio didn't respond after 5s, sending SIGKILL");
                let _ = process.kill();
                let _ = process.wait();
            }
        } else {
            tracing::warn!("AudioRecorder::stop() called but no FFmpeg process found");
        }

        tracing::info!("Audio recording stopped: {:?}", self.output_path);
        Ok(self.output_path.clone())
    }

    /// Check if recording is still running.
    pub fn is_running(&self) -> bool {
        !self.stop_flag.load(Ordering::SeqCst)
    }

    /// Get the output path.
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);

        if let Some(mut process) = self.ffmpeg_process.take() {
            // Send SIGINT for graceful shutdown on Unix (same as Ctrl+C)
            #[cfg(unix)]
            {
                let pid = Pid::from_raw(process.id() as i32);
                let _ = kill(pid, Signal::SIGINT);
            }
            
            // On Windows, use stdin 'q' command
            #[cfg(windows)]
            {
                if let Some(mut stdin) = process.stdin.take() {
                    use std::io::Write;
                    let _ = stdin.write_all(b"q");
                    let _ = stdin.flush();
                }
            }

            // Wait with timeout, then kill if still running
            if wait_with_timeout(&mut process, Duration::from_secs(2)).is_none() {
                let _ = process.kill();
                let _ = process.wait();
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn build_ffmpeg_command(output_path: &Path, config: &AudioConfig) -> Result<Command> {
    // Use ALSA for audio capture on Linux (more widely available than PulseAudio)
    let device = config.device.as_deref().unwrap_or("default");

    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-f", "alsa",
        "-i", device,
        "-ar", &config.sample_rate.to_string(),
        "-ac", &config.channels.to_string(),
        "-c:a", &config.codec,
        "-b:a", &config.bitrate,
    ])
    .arg(output_path);

    Ok(cmd)
}

#[cfg(target_os = "macos")]
fn build_ffmpeg_command(output_path: &Path, config: &AudioConfig) -> Result<Command> {
    // Use avfoundation for audio capture on macOS
    // Device index 0 is typically the default audio input
    let device = config.device.as_deref().unwrap_or(":0");

    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-f", "avfoundation",
        "-i", device,
        "-ar", &config.sample_rate.to_string(),
        "-ac", &config.channels.to_string(),
        "-c:a", &config.codec,
        "-b:a", &config.bitrate,
    ])
    .arg(output_path);

    Ok(cmd)
}

#[cfg(target_os = "windows")]
fn build_ffmpeg_command(output_path: &Path, config: &AudioConfig) -> Result<Command> {
    // Use dshow for audio capture on Windows
    let device = config.device.as_deref().unwrap_or("audio=\"Microphone\"");

    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-f", "dshow",
        "-i", device,
        "-ar", &config.sample_rate.to_string(),
        "-ac", &config.channels.to_string(),
        "-c:a", &config.codec,
        "-b:a", &config.bitrate,
    ])
    .arg(output_path);

    Ok(cmd)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn build_ffmpeg_command(_output_path: &Path, _config: &AudioConfig) -> Result<Command> {
    Err(RecorderError::Audio(
        "Audio recording not supported on this platform".to_string(),
    ))
}
