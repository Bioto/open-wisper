//! Screen recording to MP4.
//!
//! Uses scrap for screen capture and FFmpeg for encoding.

use crate::error::{RecorderError, Result};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
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

/// Configuration for screen recording.
#[derive(Debug, Clone)]
pub struct ScreenConfig {
    /// Target frames per second.
    pub fps: u32,
    /// Monitor index to capture (0 = primary).
    pub monitor: usize,
    /// Video codec (default: libx264).
    pub codec: String,
    /// Constant Rate Factor for quality (lower = better, 18-28 typical).
    pub crf: u32,
    /// Pixel format.
    pub pixel_format: String,
}

impl Default for ScreenConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            monitor: 0,
            codec: "libx264".to_string(),
            crf: 23,
            pixel_format: "yuv420p".to_string(),
        }
    }
}

/// Screen recorder handle.
pub struct ScreenRecorder {
    stop_flag: Arc<AtomicBool>,
    output_path: PathBuf,
    ffmpeg_process: Option<Child>,
    capture_thread: Option<JoinHandle<Result<()>>>,
}

impl ScreenRecorder {
    /// Start recording the screen to the given output file.
    ///
    /// Uses FFmpeg's x11grab (Linux) or avfoundation (macOS) for capture.
    pub fn start<P: AsRef<Path>>(output_path: P, config: ScreenConfig) -> Result<Self> {
        let output_path = output_path.as_ref().to_path_buf();
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Build FFmpeg command based on platform
        let mut cmd = build_ffmpeg_command(&output_path, &config)?;

        tracing::info!("Starting screen recording to {:?}", output_path);
        tracing::debug!("FFmpeg command: {:?}", cmd);

        // NOTE: We do NOT put FFmpeg in a separate process group.
        // This allows FFmpeg to receive SIGINT directly from the terminal when Ctrl+C is pressed,
        // which triggers FFmpeg's graceful shutdown (writes moov atom) before we try to stop it.

        let ffmpeg_process = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| RecorderError::Screen(format!("Failed to start FFmpeg: {}", e)))?;

        Ok(Self {
            stop_flag,
            output_path,
            ffmpeg_process: Some(ffmpeg_process),
            capture_thread: None,
        })
    }

    /// Stop the screen recording.
    pub fn stop(mut self) -> Result<PathBuf> {
        self.stop_flag.store(true, Ordering::SeqCst);
        // #region agent log - H3: Track stop sequence
        tracing::info!("ScreenRecorder::stop() called, output_path={:?}", self.output_path);
        // #endregion

        if let Some(mut process) = self.ffmpeg_process.take() {
            let pid = process.id();
            tracing::info!("FFmpeg screen process found, pid={}", pid);
            
            // Send SIGINT to FFmpeg for graceful shutdown (same as Ctrl+C).
            // SIGINT is what FFmpeg expects for interactive quit - it will flush buffers 
            // and write the moov atom. SIGTERM doesn't work well with avfoundation capture.
            #[cfg(unix)]
            {
                let nix_pid = Pid::from_raw(pid as i32);
                match kill(nix_pid, Signal::SIGINT) {
                    Ok(()) => tracing::info!("Sent SIGINT to FFmpeg screen process"),
                    Err(e) => tracing::warn!("Failed to send SIGINT to FFmpeg: {}", e),
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
            tracing::info!("Waiting for FFmpeg screen to exit (timeout=5s)...");
            if let Some(status) = wait_with_timeout(&mut process, Duration::from_secs(5)) {
                tracing::info!("FFmpeg screen exited with status: {}", status);
                if !status.success() {
                    tracing::warn!("FFmpeg screen exited with non-zero status: {}", status);
                }
            } else {
                // Timeout - FFmpeg didn't respond, force kill
                tracing::warn!("FFmpeg screen didn't respond after 5s, sending SIGKILL");
                let _ = process.kill();
                let _ = process.wait();
            }
        } else {
            tracing::warn!("ScreenRecorder::stop() called but no FFmpeg process found");
        }

        if let Some(handle) = self.capture_thread.take() {
            handle
                .join()
                .map_err(|_| RecorderError::Screen("Capture thread panicked".to_string()))??;
        }

        tracing::info!("Screen recording stopped: {:?}", self.output_path);
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

impl Drop for ScreenRecorder {
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
fn build_ffmpeg_command(output_path: &Path, config: &ScreenConfig) -> Result<Command> {
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());

    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-f", "x11grab",
        "-framerate", &config.fps.to_string(),
        "-i", &display,
        "-c:v", &config.codec,
        "-crf", &config.crf.to_string(),
        "-pix_fmt", &config.pixel_format,
        "-preset", "ultrafast",
    ])
    .arg(output_path);

    Ok(cmd)
}

#[cfg(target_os = "macos")]
fn build_ffmpeg_command(output_path: &Path, config: &ScreenConfig) -> Result<Command> {
    let fps_str = config.fps.to_string();
    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-f", "avfoundation",
        "-framerate", &fps_str,
        "-i", &format!("{}:", config.monitor),
        "-c:v", &config.codec,
        "-crf", &config.crf.to_string(),
        "-pix_fmt", &config.pixel_format,
        "-preset", "ultrafast",
        // Force output framerate to match input (avfoundation can mess this up)
        "-r", &fps_str,
        // Use fragmented MP4 so moov atom is written progressively.
        // This ensures the file is playable even if FFmpeg is killed,
        // which is necessary because avfoundation blocks signal handling.
        "-movflags", "frag_keyframe+empty_moov+default_base_moof",
    ])
    .arg(output_path);

    Ok(cmd)
}

#[cfg(target_os = "windows")]
fn build_ffmpeg_command(output_path: &Path, config: &ScreenConfig) -> Result<Command> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-f", "gdigrab",
        "-framerate", &config.fps.to_string(),
        "-i", "desktop",
        "-c:v", &config.codec,
        "-crf", &config.crf.to_string(),
        "-pix_fmt", &config.pixel_format,
        "-preset", "ultrafast",
    ])
    .arg(output_path);

    Ok(cmd)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn build_ffmpeg_command(_output_path: &Path, _config: &ScreenConfig) -> Result<Command> {
    Err(RecorderError::Screen(
        "Screen recording not supported on this platform".to_string(),
    ))
}
