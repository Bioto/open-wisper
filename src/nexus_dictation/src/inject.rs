//! Text injection into the focused application.

use crate::config::InjectMode;
use crate::error::{DictationError, Result};
use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

/// Inject transcribed text into the currently focused application.
pub struct TextInjector {
    enigo: Enigo,
    clipboard: Clipboard,
    mode: InjectMode,
    paste_threshold: usize,
}

impl TextInjector {
    /// Create an injector with the given mode.
    pub fn new(mode: InjectMode, paste_threshold: usize) -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| DictationError::Inject(format!("enigo init: {e}")))?;
        let clipboard = Clipboard::new()
            .map_err(|e| DictationError::Inject(format!("clipboard init: {e}")))?;

        Ok(Self {
            enigo,
            clipboard,
            mode,
            paste_threshold,
        })
    }

    /// Inject text using the configured strategy.
    pub fn inject(&mut self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        match self.mode {
            InjectMode::Type => self.type_text(text),
            InjectMode::Paste => self.paste_text(text),
            InjectMode::Auto => {
                if text.len() >= self.paste_threshold {
                    self.paste_text(text)
                } else {
                    self.type_text(text)
                }
            }
        }
    }

    fn type_text(&mut self, text: &str) -> Result<()> {
        for ch in text.chars() {
            self.enigo
                .text(&ch.to_string())
                .map_err(|e| DictationError::Inject(format!("type char: {e}")))?;
            thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    }

    fn paste_text(&mut self, text: &str) -> Result<()> {
        let original = self.clipboard.get_text().ok();

        self.clipboard
            .set_text(text.to_string())
            .map_err(|e| DictationError::Inject(format!("clipboard set: {e}")))?;

        thread::sleep(Duration::from_millis(50));

        #[cfg(target_os = "macos")]
        self.press_paste(Key::Meta)?;

        #[cfg(not(target_os = "macos"))]
        self.press_paste(Key::Control)?;

        thread::sleep(Duration::from_millis(100));

        if let Some(prev) = original {
            let _ = self.clipboard.set_text(prev);
        }

        Ok(())
    }

    fn press_paste(&mut self, modifier: Key) -> Result<()> {
        self.enigo
            .key(modifier, Direction::Press)
            .map_err(|e| DictationError::Inject(format!("modifier press: {e}")))?;
        self.enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| DictationError::Inject(format!("v key: {e}")))?;
        self.enigo
            .key(modifier, Direction::Release)
            .map_err(|e| DictationError::Inject(format!("modifier release: {e}")))?;
        Ok(())
    }
}
