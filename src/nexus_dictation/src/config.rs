//! Application configuration loaded from a platform config directory.

use crate::error::{DictationError, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// How text is injected into the focused application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InjectMode {
    /// Type characters via simulated keystrokes.
    Type,
    /// Paste via clipboard (Ctrl+V / Cmd+V). More reliable on Wayland.
    #[default]
    Paste,
    /// Type short text, paste long text.
    Auto,
}

/// Push-to-talk vs toggle recording activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyMode {
    /// Hold hotkey to record, release to transcribe.
    #[default]
    PushToTalk,
    /// Press once to start, press again to stop.
    Toggle,
}

/// ASR backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AsrProvider {
    #[default]
    Groq,
    #[cfg(feature = "local-asr")]
    Local,
}

/// Hotkey configuration (modifier + key name).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    /// Modifier keys: "ctrl", "alt", "shift", "super" (comma-separated).
    pub modifiers: String,
    /// Key name, e.g. "Space", "KeyD".
    pub key: String,
    pub mode: HotkeyMode,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            modifiers: "alt,shift".to_string(),
            key: "KeyZ".to_string(),
            mode: HotkeyMode::PushToTalk,
        }
    }
}

/// Cloud/local ASR settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AsrConfig {
    pub provider: AsrProvider,
    /// Groq Whisper model id.
    pub model: String,
    /// ISO-639-1 language code. Empty = auto-detect.
    pub language: String,
    /// Optional API key override (prefer `GROQ_API_KEY` env).
    pub api_key: Option<String>,
    /// Transcription API URL.
    pub api_url: String,
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            provider: AsrProvider::Groq,
            model: "whisper-large-v3-turbo".to_string(),
            language: "en".to_string(),
            api_key: None,
            api_url: "https://api.groq.com/openai/v1/audio/transcriptions".to_string(),
        }
    }
}

/// LLM rewrite settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    pub enabled: bool,
    pub model: String,
    /// Optional API key override (prefer `GROQ_API_KEY` env).
    pub api_key: Option<String>,
    /// Chat completions API URL.
    pub api_url: String,
    /// System prompt for rewriting dictated speech.
    pub system_prompt: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            model: "llama-3.3-70b-versatile".to_string(),
            api_key: None,
            api_url: "https://api.groq.com/openai/v1/chat/completions".to_string(),
            system_prompt: "Clean up dictated speech into clear, polished writing. \
                Remove filler words and fix grammar. \
                Output only the cleaned text, no quotes or commentary."
                .to_string(),
        }
    }
}

/// Local Whisper ggml model selection (`local-asr` feature only).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelConfig {
    pub repo: String,
    pub filename: String,
    pub path: Option<PathBuf>,
    pub language: String,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            repo: "ggerganov/whisper.cpp".to_string(),
            filename: "ggml-base.en.bin".to_string(),
            path: None,
            language: "en".to_string(),
        }
    }
}

/// Root application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub hotkey: HotkeyConfig,
    #[serde(default)]
    pub asr: AsrConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub model: ModelConfig,
    #[serde(default)]
    pub inject_mode: InjectMode,
    #[serde(default = "default_paste_threshold")]
    pub paste_threshold: usize,
    pub audio_device: Option<String>,
    /// Deprecated: ignored. The status overlay was replaced by the dictation HUD.
    #[serde(default)]
    pub show_overlay: bool,
}

fn default_paste_threshold() -> usize {
    80
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey: HotkeyConfig::default(),
            asr: AsrConfig::default(),
            llm: LlmConfig::default(),
            model: ModelConfig::default(),
            inject_mode: InjectMode::default(),
            paste_threshold: 80,
            audio_device: None,
            show_overlay: false,
        }
    }
}

impl AppConfig {
    /// Platform config directory, e.g. `~/.config/open-wisper`.
    pub fn config_dir() -> Result<PathBuf> {
        ProjectDirs::from("com", "open-wisper", "open-wisper")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .ok_or_else(|| DictationError::Config("could not resolve config directory".into()))
    }

    /// Path to `config.toml`.
    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Load config from disk, creating defaults if missing.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let contents = fs::read_to_string(&path)
                .map_err(|e| DictationError::Config(format!("read {}: {e}", path.display())))?;
            toml::from_str(&contents)
                .map_err(|e| DictationError::Config(format!("parse config: {e}")))
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Persist config to disk.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)
            .map_err(|e| DictationError::Config(format!("serialize config: {e}")))?;
        fs::write(&path, contents)?;
        Ok(())
    }

    /// Resolve Groq API key from env or config.
    pub fn groq_api_key(&self) -> Result<String> {
        std::env::var("GROQ_API_KEY")
            .ok()
            .or_else(|| self.asr.api_key.clone())
            .or_else(|| self.llm.api_key.clone())
            .ok_or_else(|| {
                DictationError::Config(
                    "GROQ_API_KEY not set and no api_key in [asr] or [llm] config".into(),
                )
            })
    }

    /// ASR language for transcription.
    pub fn asr_language(&self) -> Option<String> {
        let lang = self.asr.language.trim();
        if lang.is_empty() {
            None
        } else {
            Some(lang.to_string())
        }
    }

    /// Directory for cached local Whisper models.
    pub fn models_dir() -> Result<PathBuf> {
        let dir = Self::config_dir()?.join("models");
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Resolved local model path (`local-asr` feature).
    pub fn model_path(&self) -> PathBuf {
        if let Some(path) = &self.model.path {
            if path.exists() {
                return path.clone();
            }
        }
        Self::models_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(&self.model.filename)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn default_config_roundtrip() {
        let config = AppConfig::default();
        let serialized = toml::to_string(&config).unwrap();
        let parsed: AppConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(parsed.hotkey.key, "KeyZ");
        assert_eq!(parsed.asr.model, "whisper-large-v3-turbo");
        assert!(parsed.llm.enabled);
    }

    #[test]
    fn minimal_config_sections_use_defaults() {
        let toml = r#"
inject_mode = "paste"
paste_threshold = 80
show_overlay = false

[hotkey]
modifiers = "alt,shift"
key = "KeyZ"
mode = "push_to_talk"

[asr]
provider = "groq"
model = "whisper-large-v3-turbo"
language = "en"

[llm]
enabled = true
model = "llama-3.3-70b-versatile"
"#;
        let parsed: AppConfig = toml::from_str(toml).unwrap();
        assert_eq!(
            parsed.asr.api_url,
            "https://api.groq.com/openai/v1/audio/transcriptions"
        );
        assert_eq!(
            parsed.llm.api_url,
            "https://api.groq.com/openai/v1/chat/completions"
        );
        assert!(parsed.llm.system_prompt.contains("Clean up dictated speech"));
    }
}
