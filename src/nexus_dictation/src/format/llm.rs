//! LLM-based transcript formatting via Groq chat completions.

use crate::config::LlmConfig;
use crate::error::{DictationError, Result};
use crate::format::TextFormatter;
use std::time::Instant;

/// Format transcripts via Groq (OpenAI-compatible) chat completions.
pub struct LlmFormatter {
    client: reqwest::blocking::Client,
    api_url: String,
    api_key: String,
    model: String,
    system_prompt: String,
}

impl LlmFormatter {
    /// Create a formatter from LLM config and API key.
    pub fn from_config(config: &LlmConfig, api_key: String) -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            api_url: config.api_url.clone(),
            api_key,
            model: config.model.clone(),
            system_prompt: config.system_prompt.clone(),
        }
    }
}

impl TextFormatter for LlmFormatter {
    fn format(&self, text: &str) -> Result<String> {
        if text.trim().is_empty() {
            return Ok(String::new());
        }

        let started = Instant::now();

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": self.system_prompt
                },
                {
                    "role": "user",
                    "content": text
                }
            ],
            "temperature": 0.2
        });

        let response = self
            .client
            .post(&self.api_url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .map_err(|e| DictationError::Format(format!("LLM request: {e}")))?;

        let status = response.status();
        let response_body = response
            .text()
            .map_err(|e| DictationError::Format(format!("LLM response body: {e}")))?;

        if !status.is_success() {
            return Err(DictationError::Format(format!(
                "LLM rewrite failed ({status}): {response_body}"
            )));
        }

        let json: serde_json::Value = serde_json::from_str(&response_body)
            .map_err(|e| DictationError::Format(format!("LLM json: {e}")))?;

        let cleaned = json["choices"][0]["message"]["content"]
            .as_str()
            .map(str::trim)
            .map(str::to_string)
            .ok_or_else(|| DictationError::Format("missing LLM content".into()))?;

        tracing::info!(
            elapsed_ms = started.elapsed().as_millis(),
            chars = cleaned.len(),
            "LLM rewrite complete"
        );

        Ok(cleaned)
    }
}
