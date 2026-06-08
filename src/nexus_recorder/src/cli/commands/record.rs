//! Combined recording CLI command.

use crate::error::Result;
use crate::recording::{
    AudioConfig, AudioRecorder, ClickOverlayConfig, InputCapture, InputConfig,
    ScreenConfig, ScreenRecorder, overlay_clicks,
};
use clap::Args;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Arguments for the combined recording command.
#[derive(Args)]
pub struct RecordArgs {
    /// Output directory for recordings
    #[arg(short, long, default_value = ".")]
    pub output_dir: PathBuf,

    /// Record screen
    #[arg(long, default_value = "true")]
    pub screen: bool,

    /// Record audio
    #[arg(long, default_value = "true")]
    pub audio: bool,

    /// Capture input events
    #[arg(long, default_value = "true")]
    pub input: bool,

    /// Frames per second for screen recording
    #[arg(long, default_value = "30")]
    pub fps: u32,

    /// Monitor index for screen recording
    #[arg(long, default_value = "0")]
    pub monitor: usize,

    /// Audio device name
    #[arg(long)]
    pub audio_device: Option<String>,

    /// Capture mouse movement (generates many events)
    #[arg(long, default_value = "false")]
    pub mouse_moves: bool,

    /// Recording duration in seconds (0 = until Ctrl+C)
    #[arg(short, long, default_value = "0")]
    pub duration: u64,

    /// Overlay click indicators on the video after recording
    #[arg(long, default_value = "false")]
    pub overlay_clicks: bool,
}

/// Run the combined recording command.
pub async fn run_record(args: RecordArgs) -> Result<()> {
    // Create output directory
    std::fs::create_dir_all(&args.output_dir)?;

    println!("Starting recording session in {:?}", args.output_dir);
    println!("  Screen: {}, Audio: {}, Input: {}", args.screen, args.audio, args.input);
    println!("Press Ctrl+C to stop.");

    // Start recorders
    let mut screen_recorder = if args.screen {
        let path = args.output_dir.join("screen.mp4");
        let config = ScreenConfig {
            fps: args.fps,
            monitor: args.monitor,
            ..Default::default()
        };
        Some(ScreenRecorder::start(&path, config)?)
    } else {
        None
    };

    let mut audio_recorder = if args.audio {
        let path = args.output_dir.join("audio.mp4");
        let config = AudioConfig {
            device: args.audio_device,
            ..Default::default()
        };
        Some(AudioRecorder::start(&path, config)?)
    } else {
        None
    };

    let mut input_capture = if args.input {
        let path = args.output_dir.join("events.jsonl");
        let config = InputConfig {
            capture_mouse_moves: args.mouse_moves,
            ..Default::default()
        };
        Some(InputCapture::start(&path, config)?)
    } else {
        None
    };

    // Set up Ctrl+C handler
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = Arc::clone(&stop_flag);

    ctrlc::set_handler(move || {
        println!("\nStopping recording session...");
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

    // Stop all recorders
    println!("Stopping recorders...");

    let has_input = input_capture.is_some();
    let has_screen = screen_recorder.is_some();

    if let Some(capture) = input_capture.take() {
        capture.stop()?;
        println!("  Input capture saved to: {:?}", args.output_dir.join("events.jsonl"));
    }

    if let Some(recorder) = audio_recorder.take() {
        let path = recorder.stop()?;
        println!("  Audio saved to: {:?}", path);
    }

    if let Some(recorder) = screen_recorder.take() {
        let path = recorder.stop()?;
        println!("  Screen saved to: {:?}", path);
    }

    // Post-process: overlay clicks on video if requested
    if args.overlay_clicks && has_screen && has_input {
        println!("\nPost-processing: overlaying click indicators on video...");
        
        let video_path = args.output_dir.join("screen.mp4");
        let events_path = args.output_dir.join("events.jsonl");
        let output_path = args.output_dir.join("screen_with_clicks.mp4");
        
        match overlay_clicks(&video_path, &events_path, &output_path, &ClickOverlayConfig::default()) {
            Ok(()) => {
                println!("  Click overlay video saved to: {:?}", output_path);
            }
            Err(e) => {
                println!("  Warning: Failed to overlay clicks: {}", e);
            }
        }
    }

    println!("\nRecording session complete!");
    Ok(())
}
