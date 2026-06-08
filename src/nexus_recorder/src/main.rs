//! Nexus Recorder CLI - Desktop recording for screen, audio, and input capture.

use clap::Parser;
use nexus_recorder::cli::{Cli, Commands};
use nexus_recorder::error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Screen(args) => nexus_recorder::cli::run_screen(args).await,
        Commands::Audio(args) => nexus_recorder::cli::run_audio(args).await,
        Commands::Capture(args) => nexus_recorder::cli::run_capture(args).await,
        Commands::Record(args) => nexus_recorder::cli::run_record(args).await,
        Commands::ListDevices => nexus_recorder::cli::run_list_devices().await,
    }
}
