//! Input capture CLI command.

use crate::error::Result;
use crate::recording::{InputCapture, InputConfig};
use clap::Args;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Arguments for the input capture command.
#[derive(Args)]
pub struct CaptureArgs {
    /// Output file path (e.g., events.jsonl)
    #[arg(short, long, default_value = "events.jsonl")]
    pub output: String,

    /// Capture keyboard events
    #[arg(long, default_value = "true")]
    pub keyboard: bool,

    /// Capture mouse click events
    #[arg(long, default_value = "true")]
    pub mouse_clicks: bool,

    /// Capture mouse movement events
    #[arg(long, default_value = "false")]
    pub mouse_moves: bool,

    /// Capture duration in seconds (0 = until Ctrl+C)
    #[arg(short, long, default_value = "0")]
    pub duration: u64,
}

/// Run the input capture command.
pub async fn run_capture(args: CaptureArgs) -> Result<()> {
    let config = InputConfig {
        capture_keyboard: args.keyboard,
        capture_mouse_clicks: args.mouse_clicks,
        capture_mouse_moves: args.mouse_moves,
        ..Default::default()
    };

    println!("Starting input capture to {}...", args.output);
    println!("Capturing: keyboard={}, clicks={}, moves={}", 
        args.keyboard, args.mouse_clicks, args.mouse_moves);
    println!("Press Ctrl+C to stop.");

    let capture = InputCapture::start(&args.output, config)?;

    // Set up Ctrl+C handler
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = Arc::clone(&stop_flag);

    ctrlc::set_handler(move || {
        println!("\nStopping capture...");
        stop_flag_clone.store(true, Ordering::SeqCst);
    })
    .expect("Failed to set Ctrl+C handler");

    // Wait for stop signal or duration
    if args.duration > 0 {
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(args.duration)) => {
                println!("Duration reached, stopping...");
            }
            _ = async {
                while !stop_flag.load(Ordering::SeqCst) {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            } => {}
        }
    } else {
        while !stop_flag.load(Ordering::SeqCst) {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    capture.stop()?;
    println!("Events saved to: {}", args.output);

    Ok(())
}
