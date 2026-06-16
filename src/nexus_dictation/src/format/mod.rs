//! Text processing after transcription.

mod llm;

use crate::config::AppConfig;
use crate::error::Result;

pub use llm::LlmFormatter;

/// Post-process transcribed text before injection.
pub trait TextFormatter: Send + Sync {
    /// Process raw transcript text and return what to inject.
    fn format(&self, text: &str) -> Result<String>;
}

/// Pass-through formatter.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpFormatter;

impl TextFormatter for NoOpFormatter {
    fn format(&self, text: &str) -> Result<String> {
        Ok(text.trim().to_string())
    }
}

/// Basic rule-based cleanup: capitalize first letter, ensure trailing punctuation.
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicFormatter;

impl TextFormatter for BasicFormatter {
    fn format(&self, text: &str) -> Result<String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }

        let mut out = trimmed.to_string();
        if let Some(first) = out.chars().next() {
            if first.is_lowercase() {
                out.replace_range(..first.len_utf8(), &first.to_uppercase().to_string());
            }
        }

        if !out.ends_with(['.', '!', '?']) {
            out.push('.');
        }

        Ok(out)
    }
}

/// Build the configured text formatter.
pub fn build_formatter(config: &AppConfig) -> Result<Box<dyn TextFormatter>> {
    if config.llm.enabled {
        let api_key = config.groq_api_key()?;
        Ok(Box::new(LlmFormatter::from_config(&config.llm, api_key)))
    } else {
        Ok(Box::new(BasicFormatter))
    }
}
