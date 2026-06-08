//! Audio recording CLI command.

use crate::error::Result;
use crate::recording::{AudioConfig, AudioRecorder};
use clap::Args;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Arguments for the audio recording command.
#[derive(Args)]
pub struct AudioArgs {
    /// Output file path (e.g., audio.mp4)
    #[arg(short, long, default_value = "audio.mp4")]
    pub output: String,

    /// Audio device name (default: system default)
    #[arg(short, long)]
    pub device: Option<String>,

    /// Sample rate in Hz
    #[arg(short, long, default_value = "44100")]
    pub sample_rate: u32,

    /// Number of audio channels
    #[arg(short, long, default_value = "2")]
    pub channels: u16,

    /// Recording duration in seconds (0 = until Ctrl+C)
    #[arg(long, default_value = "0")]
    pub duration: u64,
}

/// Run the audio recording command.
pub async fn run_audio(args: AudioArgs) -> Result<()> {
    let config = AudioConfig {
        device: args.device,
        sample_rate: args.sample_rate,
        channels: args.channels,
        ..Default::default()
    };

    println!("Starting audio recording to {}...", args.output);
    println!("Press Ctrl+C to stop.");

    let recorder = AudioRecorder::start(&args.output, config)?;

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
