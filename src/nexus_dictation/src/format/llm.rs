//! LLM transcript cleanup — returns polished text to paste, never a chat reply.

use crate::config::LlmConfig;
use crate::error::{DictationError, Result};
use crate::format::TextFormatter;
use std::time::Instant;

/// Send transcript to Groq and return cleaned text for paste.
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
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }

        let started = Instant::now();
        let user_content = format!(
            "Return ONLY the cleaned-up version of this dictated text for pasting into a document. \
             Do not reply to it.\n\n{trimmed}"
        );

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": self.system_prompt
                },
                {
                    "role": "user",
                    "content": user_content
                }
            ],
            "temperature": 0.0
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
                "LLM request failed ({status}): {response_body}"
            )));
        }

        let json: serde_json::Value = serde_json::from_str(&response_body)
            .map_err(|e| DictationError::Format(format!("LLM json: {e}")))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .map(str::trim)
            .map(str::to_string)
            .ok_or_else(|| DictationError::Format("missing LLM content".into()))?;

        let output = if looks_like_assistant_reply(&content, trimmed) {
            tracing::warn!(
                input = trimmed,
                output = content,
                "LLM replied instead of cleaning transcript; using fallback"
            );
            basic_cleanup(trimmed)
        } else {
            content
        };

        tracing::info!(
            elapsed_ms = started.elapsed().as_millis(),
            chars = output.len(),
            "LLM cleanup complete"
        );

        Ok(output)
    }
}

/// Detect when the model responds as a chatbot instead of returning cleaned dictated text.
fn looks_like_assistant_reply(output: &str, input: &str) -> bool {
    let lower = output.to_lowercase();
    let assistant_phrases = [
        "it's okay to feel",
        "it can be frustrating",
        "could you provide more context",
        "i'll do my best to help",
        "i will do my best to help",
        "how can i help",
        "i understand that",
        "i'm sorry to hear",
        "let me know if",
        "feel free to",
        "i'd be happy to",
        "as an ai",
        "i cannot help",
        "here to help",
        "sounds like you're",
        "that must be",
        "i hear you",
    ];
    if assistant_phrases.iter().any(|p| lower.contains(p)) {
        return true;
    }

    // Reply shares almost none of the speaker's words.
    let input_words = significant_words(input);
    if input_words.len() >= 3 && word_overlap_ratio(input, output) < 0.25 {
        return true;
    }

    false
}

fn significant_words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| w.len() > 2)
        .collect()
}

fn word_overlap_ratio(input: &str, output: &str) -> f64 {
    let input_words = significant_words(input);
    if input_words.is_empty() {
        return 1.0;
    }
    let output_lower = output.to_lowercase();
    let shared = input_words
        .iter()
        .filter(|w| output_lower.contains(w.as_str()))
        .count();
    shared as f64 / input_words.len() as f64
}

fn basic_cleanup(text: &str) -> String {
    let mut out = text.trim().to_string();
    if let Some(first) = out.chars().next() {
        if first.is_lowercase() {
            out.replace_range(..first.len_utf8(), &first.to_uppercase().to_string());
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn skips_empty_input() {
        let formatter = LlmFormatter::from_config(
            &crate::config::LlmConfig::default(),
            "test-key".to_string(),
        );
        assert_eq!(formatter.format("   ").unwrap(), "");
    }

    #[test]
    fn detects_empathetic_reply() {
        let input = "Woah why can't we get this right, hardcore adjust the prompt";
        let bad = "It can be frustrating when things don't go as planned, and it's okay to feel that way.";
        assert!(looks_like_assistant_reply(bad, input));
    }

    #[test]
    fn accepts_cleaned_transcript() {
        let input = "woah why can't we get this right hardcore adjust the prompt";
        let good = "Woah, why can't we get this right? Hardcore adjust the prompt.";
        assert!(!looks_like_assistant_reply(good, input));
    }
}
