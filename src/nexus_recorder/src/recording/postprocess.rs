//! Video post-processing for overlaying click indicators.

use crate::error::{RecorderError, Result};
use crate::storage::{Event, EventType, MouseAction};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;

/// Configuration for click overlay.
#[derive(Debug, Clone)]
pub struct ClickOverlayConfig {
    /// Duration to show each click indicator (seconds).
    pub click_duration: f64,
    /// Size of the click indicator.
    pub indicator_size: u32,
    /// Border thickness.
    pub border_thickness: u32,
    /// Whether to show a ripple effect.
    pub show_ripple: bool,
    /// Ripple max size.
    pub ripple_size: u32,
}

impl Default for ClickOverlayConfig {
    fn default() -> Self {
        Self {
            click_duration: 0.4,
            indicator_size: 24,
            border_thickness: 4,
            show_ripple: true,
            ripple_size: 48,
        }
    }
}

/// A click event extracted from JSONL.
#[derive(Debug, Clone)]
struct ClickEvent {
    ts: f64,
    x: i32,
    y: i32,
    button: String,
}

/// Read click events from a JSONL file.
fn read_click_events<P: AsRef<Path>>(jsonl_path: P) -> Result<Vec<ClickEvent>> {
    let file = File::open(jsonl_path.as_ref())
        .map_err(|e| RecorderError::Storage(format!("Failed to open JSONL: {}", e)))?;
    
    let reader = BufReader::new(file);
    let mut clicks = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| RecorderError::Storage(format!("Failed to read line: {}", e)))?;
        if line.trim().is_empty() {
            continue;
        }

        // Parse the JSON line
        if let Ok(event) = serde_json::from_str::<Event>(&line) {
            if let EventType::Mouse { action, button, x, y, .. } = event.event {
                if action == MouseAction::Click {
                    clicks.push(ClickEvent {
                        ts: event.ts,
                        x,
                        y,
                        button: button.unwrap_or_else(|| "left".to_string()),
                    });
                }
            }
        }
    }

    Ok(clicks)
}

/// Build FFmpeg filter for click overlays.
/// Uses high-visibility colors with black outlines for contrast on any background.
fn build_click_filter(clicks: &[ClickEvent], config: &ClickOverlayConfig) -> String {
    if clicks.is_empty() {
        return String::new();
    }

    let mut filters = Vec::new();

    for click in clicks {
        let start = click.ts;
        let end = click.ts + config.click_duration;
        
        // High-visibility colors with good contrast
        // Yellow/orange for left, cyan for right, magenta for middle
        // All with black outlines for visibility on any background
        let (main_color, outline_color) = match click.button.as_str() {
            "right" => ("cyan", "black"),
            "middle" => ("magenta", "black"),
            _ => ("yellow", "black"),  // Left click - bright yellow
        };

        let s = config.indicator_size as i32;
        let t = config.border_thickness as i32;
        let outline = 2_i32; // Black outline thickness
        
        // Draw crosshair with black outline for contrast
        // First draw black outline (slightly larger)
        
        // Horizontal outline
        filters.push(format!(
            "drawbox=x={}:y={}:w={}:h={}:c={}:t=fill:enable='between(t,{:.16},{:.16})'",
            click.x - s - outline, click.y - t/2 - outline, 
            s * 2 + outline * 2, t + outline * 2, 
            outline_color, start, end
        ));
        
        // Vertical outline
        filters.push(format!(
            "drawbox=x={}:y={}:w={}:h={}:c={}:t=fill:enable='between(t,{:.16},{:.16})'",
            click.x - t/2 - outline, click.y - s - outline, 
            t + outline * 2, s * 2 + outline * 2, 
            outline_color, start, end
        ));
        
        // Then draw colored crosshair on top
        // Horizontal line
        filters.push(format!(
            "drawbox=x={}:y={}:w={}:h={}:c={}:t=fill:enable='between(t,{:.16},{:.16})'",
            click.x - s, click.y - t/2, s * 2, t, main_color, start, end
        ));
        
        // Vertical line
        filters.push(format!(
            "drawbox=x={}:y={}:w={}:h={}:c={}:t=fill:enable='between(t,{:.16},{:.16})'",
            click.x - t/2, click.y - s, t, s * 2, main_color, start, end
        ));
        
        // Center dot for emphasis
        let dot_size = t + 2;
        filters.push(format!(
            "drawbox=x={}:y={}:w={}:h={}:c={}:t=fill:enable='between(t,{:.16},{:.16})'",
            click.x - dot_size/2, click.y - dot_size/2, dot_size, dot_size, 
            main_color, start, end
        ));
        
        // Outer ripple ring
        if config.show_ripple {
            let rs = config.ripple_size as i32;
            
            // Draw outer square border (unfilled)
            filters.push(format!(
                "drawbox=x={}:y={}:w={}:h={}:c={}@0.8:t=3:enable='between(t,{:.16},{:.16})'",
                click.x - rs, click.y - rs, rs * 2, rs * 2, main_color, start, end
            ));
        }
    }

    filters.join(",")
}

/// Overlay click indicators on a video.
/// 
/// # Arguments
/// * `video_path` - Path to the input video
/// * `events_path` - Path to the JSONL events file
/// * `output_path` - Path for the output video with overlays
/// * `config` - Configuration for click overlays
pub fn overlay_clicks<P: AsRef<Path>>(
    video_path: P,
    events_path: P,
    output_path: P,
    config: &ClickOverlayConfig,
) -> Result<()> {
    let video_path = video_path.as_ref();
    let events_path = events_path.as_ref();
    let output_path = output_path.as_ref();

    tracing::info!("Reading click events from {:?}", events_path);
    let clicks = read_click_events(events_path)?;
    
    if clicks.is_empty() {
        tracing::info!("No click events found, copying video as-is");
        std::fs::copy(video_path, output_path)?;
        return Ok(());
    }

    tracing::info!("Found {} click events, generating overlay filter", clicks.len());
    // #region agent log - Log click details for debugging (H2, H5)
    for (i, click) in clicks.iter().enumerate().take(5) {
        tracing::info!("Click {}: ts={:.3}, x={}, y={}, button={}", i, click.ts, click.x, click.y, click.button);
    }
    // #endregion
    let filter = build_click_filter(&clicks, config);
    
    if filter.is_empty() {
        std::fs::copy(video_path, output_path)?;
        return Ok(());
    }

    tracing::info!("Applying click overlays to video");
    // #region agent log - Log filter length and sample for debugging (H1)
    tracing::info!("Filter length: {} chars, first 500: {}", filter.len(), &filter[..filter.len().min(500)]);
    // #endregion

    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-i", video_path.to_str().unwrap_or(""),
            "-vf", &filter,
            "-c:a", "copy",  // Copy audio stream
            "-c:v", "libx264",
            "-preset", "fast",
            "-crf", "23",
        ])
        .arg(output_path)
        .output()
        .map_err(|e| RecorderError::Video(format!("Failed to run FFmpeg: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // #region agent log - Capture full stderr for debugging (H4)
        tracing::error!("FFmpeg overlay stderr (full): {}", stderr);
        // #endregion
        return Err(RecorderError::Video(format!(
            "FFmpeg overlay failed: {}",
            stderr.lines().skip(10).take(30).collect::<Vec<_>>().join("\n")
        )));
    }

    tracing::info!("Click overlay complete: {:?}", output_path);
    Ok(())
}

/// Convenience function to process a recording session directory.
/// Looks for screen.mp4 and events.jsonl, outputs screen_with_clicks.mp4.
pub fn process_session_clicks<P: AsRef<Path>>(session_dir: P) -> Result<()> {
    let dir = session_dir.as_ref();
    let video_path = dir.join("screen.mp4");
    let events_path = dir.join("events.jsonl");
    let output_path = dir.join("screen_with_clicks.mp4");

    if !video_path.exists() {
        return Err(RecorderError::Video("screen.mp4 not found".to_string()));
    }

    if !events_path.exists() {
        return Err(RecorderError::Storage("events.jsonl not found".to_string()));
    }

    overlay_clicks(&video_path, &events_path, &output_path, &ClickOverlayConfig::default())
}
