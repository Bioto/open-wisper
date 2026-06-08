//! Automatic speech recognition via pluggable backends.

mod groq;

#[cfg(feature = "local-asr")]
mod whisper;

pub use groq::GroqAsr;

#[cfg(feature = "local-asr")]
pub use whisper::{download_model, load_whisper, WhisperAsr};

use crate::config::{AppConfig, AsrProvider};
use crate::error::Result;

/// Transcribe 16 kHz mono f32 PCM audio to text.
pub trait SpeechRecognizer: Send {
    /// Human-readable backend name.
    fn name(&self) -> &'static str;

    /// Transcribe audio samples.
    fn transcribe(&mut self, samples: &[f32], language: Option<&str>) -> Result<String>;
}

/// Build the configured ASR backend.
pub fn build_asr(config: &AppConfig) -> Result<Box<dyn SpeechRecognizer>> {
    match config.asr.provider {
        AsrProvider::Groq => {
            let api_key = config.groq_api_key()?;
            Ok(Box::new(GroqAsr::new(&config.asr, api_key)))
        }
        #[cfg(feature = "local-asr")]
        AsrProvider::Local => {
            let path = config.model_path();
            download_model(&config.model.repo, &config.model.filename, &path)?;
            Ok(Box::new(WhisperAsr::from_path(&path)?))
        }
    }
}

#[cfg(feature = "local-asr")]
/// Ensure a local Whisper ggml model exists, downloading via HuggingFace if needed.
pub fn ensure_model(repo: &str, filename: &str, dest: &std::path::Path) -> Result<()> {
    whisper::download_model(repo, filename, dest)
}
