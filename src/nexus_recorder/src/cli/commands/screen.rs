//! Screen recording CLI command.

use crate::error::Result;
use crate::recording::{ScreenConfig, ScreenRecorder};
use clap::Args;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Arguments for the screen recording command.
#[derive(Args)]
pub struct ScreenArgs {
    /// Output file path (e.g., output.mp4)
    #[arg(short, long, default_value = "screen.mp4")]
    pub output: String,

    /// Frames per second
    #[arg(short, long, default_value = "30")]
    pub fps: u32,

    /// Monitor index (0 = primary)
    #[arg(short, long, default_value = "0")]
    pub monitor: usize,

    /// Recording duration in seconds (0 = until Ctrl+C)
    #[arg(short, long, default_value = "0")]
    pub duration: u64,
}

/// Run the screen recording command.
pub async fn run_screen(args: ScreenArgs) -> Result<()> {
    let config = ScreenConfig {
        fps: args.fps,
        monitor: args.monitor,
        ..Default::default()
    };

    println!("Starting screen recording to {}...", args.output);
    println!("Press Ctrl+C to stop.");

    let recorder = ScreenRecorder::start(&args.output, config)?;

    // Set up Ctrl+C handler
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = Arc::clone(&stop_flag);

    ctrlc::set_handler(move || {
        println!("\nStopping recording...");
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

    let output_path = recorder.stop()?;
    println!("Recording saved to: {}", output_path.display());

    Ok(())
}
