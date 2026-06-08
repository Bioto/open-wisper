//! JSONL event writer for recording sessions.
//!
//! Writes events to a single JSONL file with timestamps synchronized to video.

use crate::error::{RecorderError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Mouse action types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MouseAction {
    Click,
    Release,
    Move,
    Scroll,
}

/// Event types that can be recorded.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventType {
    /// Session started.
    SessionStart {
        session_id: String,
        started_at: DateTime<Utc>,
    },
    /// Session ended.
    SessionEnd {
        duration: f64,
    },
    /// Keyboard event.
    Key {
        key: String,
        pressed: bool,
    },
    /// Mouse event.
    Mouse {
        action: MouseAction,
        button: Option<String>,
        x: i32,
        y: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        scroll_delta: Option<(i32, i32)>,
    },
}

/// A recorded event with timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Timestamp in seconds since recording started (syncs with video).
    pub ts: f64,
    /// The event data.
    #[serde(flatten)]
    pub event: EventType,
}

impl Event {
    /// Create a new event.
    pub fn new(ts: f64, event: EventType) -> Self {
        Self { ts, event }
    }

    /// Create a session start event.
    pub fn session_start(session_id: String) -> Self {
        Self {
            ts: 0.0,
            event: EventType::SessionStart {
                session_id,
                started_at: Utc::now(),
            },
        }
    }

    /// Create a session end event.
    pub fn session_end(duration: f64) -> Self {
        Self {
            ts: duration,
            event: EventType::SessionEnd { duration },
        }
    }

    /// Create a key event.
    pub fn key(ts: f64, key: String, pressed: bool) -> Self {
        Self {
            ts,
            event: EventType::Key { key, pressed },
        }
    }

    /// Create a mouse click event.
    pub fn mouse_click(ts: f64, button: String, x: i32, y: i32) -> Self {
        Self {
            ts,
            event: EventType::Mouse {
                action: MouseAction::Click,
                button: Some(button),
                x,
                y,
                scroll_delta: None,
            },
        }
    }

    /// Create a mouse release event.
    pub fn mouse_release(ts: f64, button: String, x: i32, y: i32) -> Self {
        Self {
            ts,
            event: EventType::Mouse {
                action: MouseAction::Release,
                button: Some(button),
                x,
                y,
                scroll_delta: None,
            },
        }
    }

    /// Create a mouse move event.
    pub fn mouse_move(ts: f64, x: i32, y: i32) -> Self {
        Self {
            ts,
            event: EventType::Mouse {
                action: MouseAction::Move,
                button: None,
                x,
                y,
                scroll_delta: None,
            },
        }
    }
}

/// Format an event with high-precision timestamp (16 decimal places).
fn format_event_with_precision(event: &Event) -> Result<String> {
    // Serialize the event type to get the rest of the JSON
    let event_json = serde_json::to_string(&event.event)?;
    
    // The event_json starts with {"type":...}, we need to inject ts at the beginning
    // Remove the leading '{' and add our high-precision ts
    let event_content = &event_json[1..]; // Skip the opening brace
    
    // Format timestamp with 16 decimal places
    let ts_formatted = format!("{:.16}", event.ts);
    
    Ok(format!("{{\"ts\":{}{}", ts_formatted, 
        if event_content.starts_with('}') { 
            "}".to_string() 
        } else { 
            format!(",{}", event_content) 
        }
    ))
}

/// Thread-safe JSONL event writer.
#[derive(Clone)]
pub struct EventWriter {
    inner: Arc<Mutex<EventWriterInner>>,
}

struct EventWriterInner {
    writer: BufWriter<File>,
    event_count: u64,
}

impl EventWriter {
    /// Create a new event writer for the given path.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path.as_ref())
            .map_err(|e| RecorderError::Storage(format!("Failed to create JSONL file: {}", e)))?;

        let writer = BufWriter::new(file);

        Ok(Self {
            inner: Arc::new(Mutex::new(EventWriterInner {
                writer,
                event_count: 0,
            })),
        })
    }

    /// Write an event to the JSONL file with high-precision timestamp.
    pub fn write(&self, event: &Event) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| RecorderError::Storage(format!("Lock poisoned: {}", e)))?;

        let line = format_event_with_precision(event)?;
        writeln!(inner.writer, "{}", line)?;
        inner.event_count += 1;

        Ok(())
    }

    /// Write multiple events.
    pub fn write_batch(&self, events: &[Event]) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| RecorderError::Storage(format!("Lock poisoned: {}", e)))?;

        for event in events {
            let line = format_event_with_precision(event)?;
            writeln!(inner.writer, "{}", line)?;
            inner.event_count += 1;
        }

        Ok(())
    }

    /// Flush the writer to disk.
    pub fn flush(&self) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|e| RecorderError::Storage(format!("Lock poisoned: {}", e)))?;

        inner.writer.flush()?;
        Ok(())
    }

    /// Get the number of events written.
    pub fn event_count(&self) -> u64 {
        self.inner
            .lock()
            .map(|inner| inner.event_count)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_event_serialization() {
        let event = Event::key(1.5, "A".to_string(), true);
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"ts\":1.5"));
        assert!(json.contains("\"type\":\"key\""));
        assert!(json.contains("\"key\":\"A\""));
        assert!(json.contains("\"pressed\":true"));
    }

    #[test]
    fn test_event_writer() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");

        let writer = EventWriter::new(&path).unwrap();
        writer
            .write(&Event::session_start("test-session".to_string()))
            .unwrap();
        writer
            .write(&Event::key(0.5, "A".to_string(), true))
            .unwrap();
        writer.write(&Event::session_end(1.0)).unwrap();
        writer.flush().unwrap();

        assert_eq!(writer.event_count(), 3);

        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 3);
    }
}
