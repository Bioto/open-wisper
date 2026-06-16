//! Dictation pipeline state machine.

use crate::asr::SpeechRecognizer;
use crate::audio::{AudioBuffer, MicCapture};
use crate::config::AppConfig;
use crate::config::HotkeyMode;
use crate::error::{DictationError, Result};
use crate::format::TextFormatter;
use crate::hotkey::{poll_hotkey_event, register_hotkey, DictationHotkey};
use crate::inject::TextInjector;
use global_hotkey::HotKeyState;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Pipeline lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DictationState {
    #[default]
    Idle,
    Recording,
    Transcribing,
    Injecting,
}

/// Events emitted to the UI layer.
#[derive(Debug, Clone)]
pub enum PipelineUiEvent {
    StateChanged(DictationState),
    Transcript(String),
    Error(String),
    Ready,
}

/// Orchestrates audio capture, ASR, formatting, and injection.
pub struct DictationPipeline {
    config: AppConfig,
    state: Arc<Mutex<DictationState>>,
    mic: Option<MicCapture>,
    buffer: Arc<AudioBuffer>,
    asr: Arc<Mutex<Box<dyn SpeechRecognizer>>>,
    formatter: Arc<dyn TextFormatter>,
    hotkey: DictationHotkey,
    _hotkey_manager: global_hotkey::GlobalHotKeyManager,
    ui_tx: Sender<PipelineUiEvent>,
}

impl DictationPipeline {
    /// Build pipeline: register hotkey and wire ASR + formatter.
    pub fn new(
        config: AppConfig,
        asr: Arc<Mutex<Box<dyn SpeechRecognizer>>>,
        formatter: Arc<dyn TextFormatter>,
        ui_tx: Sender<PipelineUiEvent>,
    ) -> Result<Self> {
        let (manager, hotkey) = register_hotkey(&config.hotkey)?;

        let pipeline = Self {
            config,
            state: Arc::new(Mutex::new(DictationState::Idle)),
            mic: None,
            buffer: Arc::new(AudioBuffer::new()),
            asr,
            formatter,
            hotkey,
            _hotkey_manager: manager,
            ui_tx,
        };

        let _ = pipeline.ui_tx.send(PipelineUiEvent::Ready);
        Ok(pipeline)
    }

    /// Current pipeline state.
    pub fn state(&self) -> DictationState {
        *self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn set_state(&self, next: DictationState) {
        if let Ok(mut state) = self.state.lock() {
            *state = next;
        }
        let _ = self.ui_tx.send(PipelineUiEvent::StateChanged(next));
    }

    /// Poll hotkey events and advance the state machine. Call from the UI loop each frame.
    pub fn poll(&mut self) {
        let Some(key_state) = poll_hotkey_event(&self.hotkey.hotkey) else {
            return;
        };

        match self.hotkey.mode {
            HotkeyMode::PushToTalk => match key_state {
                HotKeyState::Pressed if self.state() == DictationState::Idle => self.start_recording(),
                HotKeyState::Released if self.state() == DictationState::Recording => {
                    self.stop_and_process();
                }
                _ => {}
            },
            HotkeyMode::Toggle => {
                if key_state == HotKeyState::Pressed {
                    match self.state() {
                        DictationState::Idle => self.start_recording(),
                        DictationState::Recording => self.stop_and_process(),
                        _ => {}
                    }
                }
            }
        }
    }

    fn start_recording(&mut self) {
        self.buffer.clear();
        match MicCapture::start(self.config.audio_device.as_deref()) {
            Ok(capture) => {
                self.buffer = capture.buffer();
                self.mic = Some(capture);
                self.set_state(DictationState::Recording);
                tracing::info!("recording started");
            }
            Err(e) => {
                let _ = self.ui_tx.send(PipelineUiEvent::Error(e.to_string()));
            }
        }
    }

    fn stop_and_process(&mut self) {
        self.mic = None;
        let samples = self.buffer.take_samples();
        self.set_state(DictationState::Transcribing);
        tracing::info!(samples = samples.len(), "recording stopped, transcribing");

        let asr = Arc::clone(&self.asr);
        let formatter = Arc::clone(&self.formatter);
        let language = self.config.asr_language();
        let ui_tx = self.ui_tx.clone();
        let state = Arc::clone(&self.state);
        let inject_mode = self.config.inject_mode;
        let paste_threshold = self.config.paste_threshold;

        thread::spawn(move || {
            let transcribe_started = Instant::now();
            let transcript = match asr.lock() {
                Ok(mut asr) => {
                    let lang = language.as_deref();
                    asr.as_mut().transcribe(&samples, lang)
                }
                Err(_) => Err(DictationError::Pipeline("ASR lock poisoned".into())),
            };

            match transcript {
                Ok(text) if text.is_empty() => {
                    tracing::info!("transcription empty");
                    set_idle(&state, &ui_tx);
                }
                Ok(text) => {
                    tracing::info!(
                        elapsed_ms = transcribe_started.elapsed().as_millis(),
                        raw = %text,
                        "transcription complete"
                    );
                    let _ = ui_tx.send(PipelineUiEvent::Transcript(text.clone()));

                    let rewrite_started = Instant::now();
                    match formatter.format(&text) {
                        Ok(clean) => {
                            tracing::info!(
                                elapsed_ms = rewrite_started.elapsed().as_millis(),
                                output = %clean,
                                "LLM response complete"
                            );
                            set_state(&state, &ui_tx, DictationState::Injecting);

                            let inject_started = Instant::now();
                            match inject_text(inject_mode, paste_threshold, &clean) {
                                Ok(()) => tracing::info!(
                                    elapsed_ms = inject_started.elapsed().as_millis(),
                                    "text injected"
                                ),
                                Err(e) => {
                                    let _ = ui_tx.send(PipelineUiEvent::Error(e.to_string()));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = ui_tx.send(PipelineUiEvent::Error(e.to_string()));
                        }
                    }
                    set_idle(&state, &ui_tx);
                }
                Err(e) => {
                    tracing::error!(error = %e, "transcription failed");
                    let _ = ui_tx.send(PipelineUiEvent::Error(e.to_string()));
                    set_idle(&state, &ui_tx);
                }
            }
        });
    }
}

fn set_state(state: &Arc<Mutex<DictationState>>, ui_tx: &Sender<PipelineUiEvent>, next: DictationState) {
    if let Ok(mut s) = state.lock() {
        *s = next;
    }
    let _ = ui_tx.send(PipelineUiEvent::StateChanged(next));
}

fn set_idle(state: &Arc<Mutex<DictationState>>, ui_tx: &Sender<PipelineUiEvent>) {
    set_state(state, ui_tx, DictationState::Idle);
}

fn inject_text(
    mode: crate::config::InjectMode,
    paste_threshold: usize,
    text: &str,
) -> Result<()> {
    let mut injector = TextInjector::new(mode, paste_threshold)?;
    injector.inject(text)
}

/// Headless capture, transcribe, and rewrite for CLI testing.
pub fn headless_transcribe(
    config: &AppConfig,
    asr: &mut dyn SpeechRecognizer,
    formatter: &dyn TextFormatter,
    duration: Duration,
) -> Result<String> {
    let capture = MicCapture::start(config.audio_device.as_deref())?;
    let buffer = capture.buffer();
    thread::sleep(duration);
    drop(capture);
    let samples = buffer.take_samples();

    let language = config.asr_language();
    let lang = language.as_deref();
    let text = asr.transcribe(&samples, lang)?;
    tracing::info!(raw = %text, "transcription complete");
    formatter.format(&text)
}
