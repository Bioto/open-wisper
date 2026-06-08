//! Open Wispr - voice dictation with Groq ASR and LLM rewrite.

#![allow(clippy::missing_errors_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::single_char_pattern)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::unnecessary_literal_bound)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]

pub mod asr;
pub mod audio;
pub mod config;
pub mod error;
pub mod format;
pub mod hotkey;
pub mod inject;
pub mod pipeline;
pub mod ui;

pub use config::AppConfig;
pub use error::{DictationError, Result};

use crate::asr::{build_asr, SpeechRecognizer};
use crate::format::{build_formatter, TextFormatter};
use std::sync::{Arc, Mutex};

/// Initialized services shared by CLI and GUI modes.
pub struct AppServices {
    pub config: AppConfig,
    pub asr: Arc<Mutex<Box<dyn SpeechRecognizer>>>,
    pub formatter: Arc<dyn TextFormatter>,
}

impl AppServices {
    /// Load config and initialize Groq ASR + formatter (no local model load).
    pub fn bootstrap() -> Result<Self> {
        let config = AppConfig::load()?;
        let asr = Arc::new(Mutex::new(build_asr(&config)?));
        let formatter = Arc::from(build_formatter(&config)?);

        tracing::info!(
            asr = asr.lock().map(|a| a.name()).unwrap_or("unknown"),
            llm_enabled = config.llm.enabled,
            "services ready"
        );

        Ok(Self {
            config,
            asr,
            formatter,
        })
    }
}
