//! Groq cloud Whisper transcription backend.

use crate::asr::SpeechRecognizer;
use crate::audio::WHISPER_SAMPLE_RATE;
use crate::config::AsrConfig;
use crate::error::{DictationError, Result};
use reqwest::blocking::multipart;
use std::io::Cursor;
use std::time::Instant;

const MIN_AUDIO_SECS: f32 = 0.2;

/// Groq Whisper ASR via OpenAI-compatible transcription API.
pub struct GroqAsr {
    client: reqwest::blocking::Client,
    api_key: String,
    config: AsrConfig,
}

impl GroqAsr {
    /// Create a Groq ASR client from config and API key.
    pub fn new(config: &AsrConfig, api_key: String) -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            api_key,
            config: config.clone(),
        }
    }
}

impl SpeechRecognizer for GroqAsr {
    fn name(&self) -> &'static str {
        "groq-whisper"
    }

    fn transcribe(&mut self, samples: &[f32], language: Option<&str>) -> Result<String> {
        if samples.is_empty() {
            return Ok(String::new());
        }

        let min_samples = (WHISPER_SAMPLE_RATE as f32 * MIN_AUDIO_SECS) as usize;
        let samples = if samples.len() < min_samples {
            let mut padded = samples.to_vec();
            padded.resize(min_samples, 0.0);
            padded
        } else {
            samples.to_vec()
        };

        let wav = samples_to_wav(&samples, WHISPER_SAMPLE_RATE)?;
        let started = Instant::now();

        let file_part = multipart::Part::bytes(wav)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| DictationError::Asr(format!("mime: {e}")))?;

        let mut form = multipart::Form::new()
            .part("file", file_part)
            .text("model", self.config.model.clone())
            .text("response_format", "json");

        if let Some(lang) = language.filter(|l| !l.is_empty()) {
            form = form.text("language", lang.to_string());
        }

        let response = self
            .client
            .post(&self.config.api_url)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .map_err(|e| DictationError::Asr(format!("groq request: {e}")))?;

        let status = response.status();
        let body = response
            .text()
            .map_err(|e| DictationError::Asr(format!("groq response body: {e}")))?;

        if !status.is_success() {
            return Err(DictationError::Asr(format!(
                "groq transcription failed ({status}): {body}"
            )));
        }

        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| DictationError::Asr(format!("groq json: {e}")))?;

        let text = json["text"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string();

        tracing::info!(
            elapsed_ms = started.elapsed().as_millis(),
            chars = text.len(),
            "groq transcription complete"
        );

        Ok(text)
    }
}

fn samples_to_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(&mut buffer, spec)
            .map_err(|e| DictationError::Asr(format!("wav writer: {e}")))?;
        for &sample in samples {
            let clipped = sample.clamp(-1.0, 1.0);
            let int_sample = (clipped * f32::from(i16::MAX)) as i16;
            writer
                .write_sample(int_sample)
                .map_err(|e| DictationError::Asr(format!("wav sample: {e}")))?;
        }
        writer
            .finalize()
            .map_err(|e| DictationError::Asr(format!("wav finalize: {e}")))?;
    }
    Ok(buffer.into_inner())
}
