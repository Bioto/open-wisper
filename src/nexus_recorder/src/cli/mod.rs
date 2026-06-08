//! CLI module for nexus-recorder.

pub mod commands;

use clap::{Parser, Subcommand};

/// Nexus Recorder - Desktop recording for screen, audio, and input capture
#[derive(Parser)]
#[command(name = "nexus-recorder")]
#[command(about = "Record screen, audio, and input events", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Record the screen to an MP4 file
    Screen(commands::ScreenArgs),

    /// Record audio to an MP4 file
    Audio(commands::AudioArgs),

    /// Capture keyboard and mouse input to JSONL
    Capture(commands::CaptureArgs),

    /// Combined recording (screen + audio + input)
    Record(commands::RecordArgs),

    /// List available audio devices
    ListDevices,
}

pub use commands::{run_audio, run_capture, run_record, run_screen, run_list_devices};
