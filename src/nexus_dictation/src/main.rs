//! Open Wispr CLI - voice dictation with Groq ASR and LLM rewrite.

use clap::{Parser, Subcommand};
use nexus_dictation::asr::build_asr;
use nexus_dictation::audio::list_input_devices;
use nexus_dictation::config::AppConfig;
use nexus_dictation::format::build_formatter;
use nexus_dictation::pipeline::headless_transcribe;
use nexus_dictation::AppServices;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "nexus-dictation", about = "Open Wispr - Groq voice dictation")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the tray app (default).
    Run,
    /// Record for N seconds, transcribe, rewrite, and print result.
    Test {
        /// Recording duration in seconds.
        #[arg(short, long, default_value_t = 5)]
        seconds: u64,
        /// Inject rewritten text into the focused app.
        #[arg(long, default_value_t = false)]
        inject: bool,
    },
    /// Download local Whisper model (`local-asr` feature only).
    #[cfg(feature = "local-asr")]
    DownloadModel,
    /// List cpal input devices.
    ListDevices,
    /// Print config path and current settings.
    ShowConfig,
}

fn main() -> nexus_dictation::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("nexus_dictation=info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Run) {
        Commands::Run => {
            let services = AppServices::bootstrap()?;
            nexus_dictation::ui::run_app(services)?;
        }
        Commands::Test { seconds, inject } => {
            let config = AppConfig::load()?;
            let mut asr = build_asr(&config)?;
            let formatter = build_formatter(&config)?;
            let text = headless_transcribe(
                &config,
                asr.as_mut(),
                formatter.as_ref(),
                Duration::from_secs(seconds),
            )?;
            println!("{text}");
            if inject {
                let mut injector = nexus_dictation::inject::TextInjector::new(
                    config.inject_mode,
                    config.paste_threshold,
                )?;
                injector.inject(&text)?;
            }
        }
        #[cfg(feature = "local-asr")]
        Commands::DownloadModel => {
            use nexus_dictation::asr::ensure_model;
            let config = AppConfig::load()?;
            let path = config.model_path();
            ensure_model(&config.model.repo, &config.model.filename, &path)?;
            println!("Model ready at {}", path.display());
        }
        Commands::ListDevices => {
            for name in list_input_devices()? {
                println!("{name}");
            }
        }
        Commands::ShowConfig => {
            let config = AppConfig::load()?;
            println!("Config: {}", AppConfig::config_path()?.display());
            println!("{config:#?}");
        }
    }

    Ok(())
}
