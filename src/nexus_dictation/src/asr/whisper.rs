//! whisper-rs backend for local Whisper transcription.

use crate::asr::SpeechRecognizer;
use crate::error::{DictationError, Result};
use std::path::Path;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Local Whisper ASR using whisper.cpp via whisper-rs.
pub struct WhisperAsr {
    _context: WhisperContext,
    state: WhisperState,
}

impl WhisperAsr {
    /// Load a ggml model from disk.
    pub fn from_path(model_path: &Path) -> Result<Self> {
        if !model_path.exists() {
            return Err(DictationError::Asr(format!(
                "model not found: {}",
                model_path.display()
            )));
        }

        let path_str = model_path
            .to_str()
            .ok_or_else(|| DictationError::Asr("invalid model path".into()))?;

        let context = WhisperContext::new_with_params(path_str, WhisperContextParameters::default())
            .map_err(|e| DictationError::Asr(format!("load model: {e}")))?;
        let state = context
            .create_state()
            .map_err(|e| DictationError::Asr(format!("create state: {e}")))?;

        Ok(Self {
            _context: context,
            state,
        })
    }
}

impl SpeechRecognizer for WhisperAsr {
    fn name(&self) -> &'static str {
        "whisper-rs"
    }

    fn transcribe(&mut self, samples: &[f32], language: Option<&str>) -> Result<String> {
        if samples.is_empty() {
            return Ok(String::new());
        }

        // Pad very short clips to avoid whisper edge cases.
        let min_samples = WHISPER_SAMPLE_RATE as usize / 5;
        let audio: Vec<f32> = if samples.len() < min_samples {
            let mut padded = samples.to_vec();
            padded.resize(min_samples, 0.0);
            padded
        } else {
            samples.to_vec()
        };

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(num_cpus());
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        if let Some(lang) = language.filter(|l| !l.is_empty()) {
            params.set_language(Some(lang));
        }

        self.state
            .full(params, &audio)
            .map_err(|e| DictationError::Asr(format!("inference: {e}")))?;

        let mut text = String::new();
        for segment in self.state.as_iter() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&segment.to_string());
        }

        Ok(text.trim().to_string())
    }
}

/// Download model from HuggingFace Hub (sync API).
pub fn download_model(repo: &str, filename: &str, dest: &Path) -> Result<()> {
    if dest.exists() {
        tracing::info!(path = %dest.display(), "model already present");
        return Ok(());
    }

    tracing::info!(repo, filename, "downloading Whisper model");

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let api = hf_hub::api::sync::Api::new()
        .map_err(|e| DictationError::Asr(format!("hf-hub init: {e}")))?;
    let repo_handle = api.model(repo.to_string());
    let downloaded = repo_handle
        .get(filename)
        .map_err(|e| DictationError::Asr(format!("download {filename}: {e}")))?;

    std::fs::copy(&downloaded, dest).map_err(|e| {
        DictationError::Asr(format!(
            "copy model to {}: {e}",
            dest.display()
        ))
    })?;

    tracing::info!(path = %dest.display(), "model ready");
    Ok(())
}

fn num_cpus() -> i32 {
    i32::try_from(std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get))
        .unwrap_or(4)
}

/// Resolve and load Whisper ASR, downloading the model if needed.
pub fn load_whisper(repo: &str, filename: &str, dest: &Path) -> Result<WhisperAsr> {
    download_model(repo, filename, dest)?;
    WhisperAsr::from_path(dest)
}
