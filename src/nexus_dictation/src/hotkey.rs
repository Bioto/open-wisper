//! Global hotkey registration and event handling.

use crate::config::{AppConfig, HotkeyConfig, HotkeyMode};
use crate::error::{DictationError, Result};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::str::FromStr;

/// Parsed hotkey ready for registration.
#[derive(Debug, Clone)]
pub struct DictationHotkey {
    pub hotkey: HotKey,
    pub mode: HotkeyMode,
}

/// Build a global hotkey from config.
pub fn hotkey_from_config(config: &HotkeyConfig) -> Result<DictationHotkey> {
    let modifiers = parse_modifiers(&config.modifiers)?;
    let code = parse_code(&config.key)?;
    Ok(DictationHotkey {
        hotkey: HotKey::new(Some(modifiers), code),
        mode: config.mode,
    })
}

/// Register the dictation hotkey and return the manager.
pub fn register_hotkey(config: &HotkeyConfig) -> Result<(GlobalHotKeyManager, DictationHotkey)> {
    let parsed = hotkey_from_config(config)?;
    let manager = GlobalHotKeyManager::new()
        .map_err(|e| DictationError::Hotkey(format!("init manager: {e}")))?;
    manager.register(parsed.hotkey).map_err(|e| {
        let combo = format_hotkey_label(config);
        DictationError::Hotkey(format!(
            "could not register hotkey {combo}: {e}. \
             Another app or a leftover nexus-dictation process may already own this shortcut. \
             Try `pkill nexus-dictation` or change [hotkey] in {}",
            AppConfig::config_path().map_or_else(
                |_| "~/.config/open-wisper/config.toml".into(),
                |p| p.display().to_string(),
            )
        ))
    })?;
    Ok((manager, parsed))
}

fn format_hotkey_label(config: &HotkeyConfig) -> String {
    format!(
        "{}+{}",
        config.modifiers.replace(',', "+"),
        config.key
    )
}

/// Non-blocking poll for hotkey press/release events matching our hotkey id.
pub fn poll_hotkey_event(hotkey: &HotKey) -> Option<HotKeyState> {
    GlobalHotKeyEvent::receiver()
        .try_recv()
        .ok()
        .filter(|event| event.id == hotkey.id())
        .map(|event| event.state)
}

fn parse_modifiers(raw: &str) -> Result<Modifiers> {
    let mut mods = Modifiers::empty();
    for part in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let part_lower = part.to_ascii_lowercase();
        mods |= match part_lower.as_str() {
            "ctrl" | "control" => Modifiers::CONTROL,
            "alt" | "option" => Modifiers::ALT,
            "shift" => Modifiers::SHIFT,
            "super" | "meta" | "cmd" | "command" | "win" => Modifiers::SUPER,
            other => {
                return Err(DictationError::Hotkey(format!(
                    "unknown modifier: {other}"
                )));
            }
        };
    }
    Ok(mods)
}

fn parse_code(raw: &str) -> Result<Code> {
    let normalized = if raw.starts_with("Key") || raw.starts_with("Digit") || raw.starts_with("F") {
        raw.to_string()
    } else {
        match raw.to_ascii_lowercase().as_str() {
            "space" => "Space".to_string(),
            "enter" | "return" => "Enter".to_string(),
            "tab" => "Tab".to_string(),
            "escape" | "esc" => "Escape".to_string(),
            other if other.len() == 1 => {
                let ch = other.chars().next().ok_or_else(|| {
                    DictationError::Hotkey(format!("unknown key: {raw}"))
                })?;
                if ch.is_ascii_alphabetic() {
                    format!("Key{}", ch.to_ascii_uppercase())
                } else if ch.is_ascii_digit() {
                    format!("Digit{ch}")
                } else {
                    return Err(DictationError::Hotkey(format!("unknown key: {raw}")));
                }
            }
            other => other.to_string(),
        }
    };

    Code::from_str(&normalized).map_err(|_| DictationError::Hotkey(format!("unknown key: {raw}")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_alt_shift_z() {
        let config = HotkeyConfig {
            modifiers: "alt,shift".into(),
            key: "KeyZ".into(),
            mode: HotkeyMode::PushToTalk,
        };
        let parsed = hotkey_from_config(&config).unwrap();
        assert_eq!(parsed.mode, HotkeyMode::PushToTalk);
    }
}
