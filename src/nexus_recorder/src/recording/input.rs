//! Input capture for keyboard and mouse events.
//!
//! Uses device_query for cross-platform input monitoring.

use crate::error::{RecorderError, Result};
use crate::storage::{Event, EventWriter};
use device_query::{DeviceQuery, DeviceState, Keycode, MouseState};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Configuration for input capture.
#[derive(Debug, Clone)]
pub struct InputConfig {
    /// Poll interval in milliseconds.
    pub poll_interval_ms: u64,
    /// Whether to capture keyboard events.
    pub capture_keyboard: bool,
    /// Whether to capture mouse clicks.
    pub capture_mouse_clicks: bool,
    /// Whether to capture mouse movement.
    pub capture_mouse_moves: bool,
    /// Minimum pixels moved before recording a move event.
    pub mouse_move_threshold: i32,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 10,
            capture_keyboard: true,
            capture_mouse_clicks: true,
            capture_mouse_moves: false, // Disabled by default (generates many events)
            mouse_move_threshold: 5,
        }
    }
}

/// Input capture handle for keyboard and mouse events.
pub struct InputCapture {
    stop_flag: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<Result<()>>>,
}

impl InputCapture {
    /// Start capturing input events to the given JSONL file.
    pub fn start<P: AsRef<Path>>(output_path: P, config: InputConfig) -> Result<Self> {
        let writer = EventWriter::new(output_path)?;
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = Arc::clone(&stop_flag);

        let handle = thread::spawn(move || {
            capture_loop(writer, config, stop_flag_clone)
        });

        Ok(Self {
            stop_flag,
            handle: Some(handle),
        })
    }

    /// Start capturing with events written to an existing EventWriter.
    pub fn start_with_writer(writer: EventWriter, config: InputConfig) -> Result<Self> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = Arc::clone(&stop_flag);

        let handle = thread::spawn(move || {
            capture_loop(writer, config, stop_flag_clone)
        });

        Ok(Self {
            stop_flag,
            handle: Some(handle),
        })
    }

    /// Stop the input capture.
    pub fn stop(mut self) -> Result<()> {
        self.stop_flag.store(true, Ordering::SeqCst);
        
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| RecorderError::Input("Capture thread panicked".to_string()))??;
        }
        
        Ok(())
    }

    /// Check if capture is still running.
    pub fn is_running(&self) -> bool {
        !self.stop_flag.load(Ordering::SeqCst)
    }
}

impl Drop for InputCapture {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

fn capture_loop(
    writer: EventWriter,
    config: InputConfig,
    stop_flag: Arc<AtomicBool>,
) -> Result<()> {
    let device_state = DeviceState::new();
    let start_time = Instant::now();
    let poll_interval = Duration::from_millis(config.poll_interval_ms);

    let mut prev_keys: HashSet<Keycode> = HashSet::new();
    let mut prev_mouse_buttons: Vec<bool> = vec![false; 5];
    let mut prev_mouse_pos: (i32, i32) = device_state.get_mouse().coords;

    tracing::info!("Input capture started");

    while !stop_flag.load(Ordering::SeqCst) {
        let ts = start_time.elapsed().as_secs_f64();

        // Keyboard events
        if config.capture_keyboard {
            let keys: HashSet<Keycode> = device_state.get_keys().into_iter().collect();

            // Key presses (new keys)
            for key in keys.difference(&prev_keys) {
                let event = Event::key(ts, format!("{:?}", key), true);
                writer.write(&event)?;
            }

            // Key releases (removed keys)
            for key in prev_keys.difference(&keys) {
                let event = Event::key(ts, format!("{:?}", key), false);
                writer.write(&event)?;
            }

            prev_keys = keys;
        }

        // Mouse events
        let mouse: MouseState = device_state.get_mouse();

        if config.capture_mouse_clicks {
            // Check each button (up to 5)
            for (i, &pressed) in mouse.button_pressed.iter().enumerate().take(5) {
                if i < prev_mouse_buttons.len() && pressed != prev_mouse_buttons[i] {
                    let button_name = match i {
                        0 => "left",
                        1 => "right",
                        2 => "middle",
                        3 => "button4",
                        4 => "button5",
                        _ => "unknown",
                    };

                    let event = if pressed {
                        Event::mouse_click(ts, button_name.to_string(), mouse.coords.0, mouse.coords.1)
                    } else {
                        Event::mouse_release(ts, button_name.to_string(), mouse.coords.0, mouse.coords.1)
                    };
                    writer.write(&event)?;
                }
            }
            prev_mouse_buttons = mouse.button_pressed.iter().take(5).copied().collect();
        }

        if config.capture_mouse_moves {
            let dx = (mouse.coords.0 - prev_mouse_pos.0).abs();
            let dy = (mouse.coords.1 - prev_mouse_pos.1).abs();

            if dx >= config.mouse_move_threshold || dy >= config.mouse_move_threshold {
                let event = Event::mouse_move(ts, mouse.coords.0, mouse.coords.1);
                writer.write(&event)?;
                prev_mouse_pos = mouse.coords;
            }
        }

        thread::sleep(poll_interval);
    }

    writer.flush()?;
    tracing::info!("Input capture stopped, {} events recorded", writer.event_count());

    Ok(())
}
