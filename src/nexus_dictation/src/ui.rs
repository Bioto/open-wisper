//! System tray, dictation HUD overlay, and settings window.

use crate::config::AppConfig;
use crate::pipeline::{DictationState, PipelineUiEvent};
use crate::AppServices;
use eframe::egui::{self, ViewportBuilder, ViewportCommand, ViewportId};
use std::cell::Cell;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};

fn hud_viewport_id() -> ViewportId {
    ViewportId::from_hash_of("dictation_hud")
}

fn settings_viewport_id() -> ViewportId {
    ViewportId::from_hash_of("settings")
}

/// Commands from the tray menu to the UI loop.
enum TrayCommand {
    ShowSettings,
    Quit,
}

/// Shared UI state updated by pipeline events.
#[derive(Default)]
struct UiState {
    dictation_state: DictationState,
    last_transcript: String,
    last_error: Option<String>,
    status_message: String,
}

impl UiState {
    fn new() -> Self {
        Self {
            status_message: "Ready".to_string(),
            ..Self::default()
        }
    }
}

/// Run the tray app with a hidden root window, dictation HUD, and settings pane.
pub fn run_app(services: AppServices) -> crate::error::Result<()> {
    init_platform_ui()?;

    let (ui_tx, ui_rx) = mpsc::channel();
    let (tray_tx, tray_rx) = mpsc::channel();

    let hotkey_hint = format_hotkey_hint(&services.config);
    let config_path = AppConfig::config_path().map_or_else(
        |_| "~/.config/open-wisper/config.toml".to_string(),
        |p| p.display().to_string(),
    );

    let mut pipeline = crate::pipeline::DictationPipeline::new(
        services.config.clone(),
        Arc::clone(&services.asr),
        Arc::clone(&services.formatter),
        ui_tx,
    )?;

    let _tray = create_tray_icon(tray_tx)?;

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Open Wispr")
            .with_app_id("open-wisper")
            .with_inner_size([1.0, 1.0])
            .with_decorations(false)
            .with_taskbar(false)
            .with_visible(false),
        ..Default::default()
    };

    eframe::run_simple_native("Open Wispr", native_options, move |ctx, _frame| {
        pump_platform_events();
        poll_tray_events(&tray_rx, ctx);
        drain_pipeline_events(&ui_rx, ctx);
        pipeline.poll();

        ctx.send_viewport_cmd(ViewportCommand::Visible(false));

        let hud_active = UI_STATE.with(|cell| hud_active(cell.borrow().dictation_state));
        if hud_active {
            ctx.show_viewport_deferred(hud_viewport_id(), hud_viewport_builder(), |ctx, _class| {
                if let Some(cmd) = ViewportCommand::center_on_screen(ctx) {
                    ctx.send_viewport_cmd(cmd);
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(24.0);
                            ui.add(egui::Spinner::new().size(48.0));
                        });
                    });
                ctx.request_repaint();
            });
        } else {
            ctx.send_viewport_cmd_to(hud_viewport_id(), ViewportCommand::Close);
        }

        let settings_open = SETTINGS_OPEN.get();
        if settings_open {
            let hint = hotkey_hint.clone();
            let path = config_path.clone();
            ctx.show_viewport_deferred(
                settings_viewport_id(),
                settings_viewport_builder(),
                move |ctx, _class| {
                    if ctx.input(|i| i.viewport().close_requested()) {
                        SETTINGS_OPEN.set(false);
                        return;
                    }
                    egui::CentralPanel::default().show(ctx, |ui| {
                        render_settings(ui, &hint, &path);
                    });
                },
            );
        } else {
            ctx.send_viewport_cmd_to(settings_viewport_id(), ViewportCommand::Close);
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    })
    .map_err(|e| crate::error::DictationError::Ui(e.to_string()))?;

    Ok(())
}

fn hud_active(state: DictationState) -> bool {
    matches!(
        state,
        DictationState::Recording | DictationState::Transcribing
    )
}

fn hud_viewport_builder() -> ViewportBuilder {
    let builder = ViewportBuilder::default()
        .with_title("Open Wispr")
        .with_app_id("open-wisper")
        .with_inner_size([96.0, 96.0])
        .with_min_inner_size([96.0, 96.0])
        .with_max_inner_size([96.0, 96.0])
        .with_resizable(false)
        .with_decorations(false)
        .with_taskbar(false)
        .with_always_on_top()
        .with_mouse_passthrough(true);

    #[cfg(target_os = "linux")]
    {
        builder.with_window_type(egui::X11WindowType::Utility)
    }
    #[cfg(not(target_os = "linux"))]
    {
        builder
    }
}

fn settings_viewport_builder() -> ViewportBuilder {
    ViewportBuilder::default()
        .with_title("Open Wispr Settings")
        .with_app_id("open-wisper")
        .with_inner_size([400.0, 280.0])
        .with_min_inner_size([320.0, 200.0])
        .with_decorations(true)
}

fn init_platform_ui() -> crate::error::Result<()> {
    #[cfg(target_os = "linux")]
    {
        gtk::init().map_err(|e| {
            crate::error::DictationError::Ui(format!(
                "GTK init failed ({e}). Install libgtk-3-dev and libappindicator3-dev"
            ))
        })?;
    }
    Ok(())
}

fn pump_platform_events() {
    #[cfg(target_os = "linux")]
    {
        while gtk::events_pending() {
            gtk::main_iteration();
        }
    }
}

fn create_tray_icon(tray_tx: Sender<TrayCommand>) -> crate::error::Result<tray_icon::TrayIcon> {
    let menu = build_tray_menu();
    let icon = load_tray_icon()?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Open Wispr — Alt+Shift+Z to dictate")
        .with_icon(icon)
        .build()
        .map_err(|e| crate::error::DictationError::Ui(format!("tray icon: {e}")))?;

    std::thread::spawn(move || {
        loop {
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                let cmd = match event.id.as_ref() {
                    "settings" => Some(TrayCommand::ShowSettings),
                    "quit" => Some(TrayCommand::Quit),
                    _ => None,
                };
                if let Some(cmd) = cmd {
                    let _ = tray_tx.send(cmd);
                }
            }

            while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                if matches!(event, TrayIconEvent::DoubleClick { .. }) {
                    let _ = tray_tx.send(TrayCommand::ShowSettings);
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });

    Ok(tray)
}

fn build_tray_menu() -> Menu {
    let menu = Menu::new();
    let _ = menu.append(&MenuItem::with_id("settings", "Settings", true, None));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id("quit", "Quit", true, None));
    menu
}

fn load_tray_icon() -> crate::error::Result<Icon> {
    let rgba = image::RgbaImage::from_fn(32, 32, |x, y| {
        let cx = 16.0_f32;
        let cy = 16.0_f32;
        let dist = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
        if dist < 12.0 {
            image::Rgba([90, 140, 255, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        }
    });
    Icon::from_rgba(rgba.into_raw(), 32, 32)
        .map_err(|e| crate::error::DictationError::Ui(format!("icon: {e}")))
}

fn format_hotkey_hint(config: &AppConfig) -> String {
    format!(
        "{} + {} ({:?})",
        config.hotkey.modifiers.replace(',', " + "),
        config.hotkey.key,
        config.hotkey.mode
    )
}

thread_local! {
    static UI_STATE: std::cell::RefCell<UiState> = std::cell::RefCell::new(UiState::new());
    static SETTINGS_OPEN: Cell<bool> = const { Cell::new(false) };
}

fn poll_tray_events(tray_rx: &Receiver<TrayCommand>, ctx: &egui::Context) {
    while let Ok(cmd) = tray_rx.try_recv() {
        match cmd {
            TrayCommand::ShowSettings => {
                SETTINGS_OPEN.set(true);
                ctx.request_repaint();
            }
            TrayCommand::Quit => std::process::exit(0),
        }
    }
}

fn drain_pipeline_events(ui_rx: &Receiver<PipelineUiEvent>, ctx: &egui::Context) {
    while let Ok(event) = ui_rx.try_recv() {
        UI_STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            match event {
                PipelineUiEvent::StateChanged(s) => {
                    state.dictation_state = s;
                    state.status_message = match s {
                        DictationState::Idle => "Ready".to_string(),
                        DictationState::Recording => "Recording…".to_string(),
                        DictationState::Transcribing => "Transcribing…".to_string(),
                        DictationState::Injecting => "Injecting…".to_string(),
                    };
                }
                PipelineUiEvent::Transcript(t) => state.last_transcript = t,
                PipelineUiEvent::Error(e) => state.last_error = Some(e),
                PipelineUiEvent::Ready => state.status_message = "Ready".to_string(),
            }
        });
        ctx.request_repaint();
    }
}

fn render_settings(ui: &mut egui::Ui, hotkey_hint: &str, config_path: &str) {
    ui.heading("Open Wispr");
    ui.add_space(8.0);
    ui.label(format!("Hotkey: {hotkey_hint}"));
    ui.add_space(4.0);
    ui.label("Config:");
    ui.monospace(config_path);
    ui.add_space(8.0);
    ui.label("Edit config.toml and restart to change settings.");

    UI_STATE.with(|cell| {
        let state = cell.borrow();
        if let Some(err) = &state.last_error {
            ui.add_space(8.0);
            ui.separator();
            ui.colored_label(egui::Color32::RED, format!("Last error: {err}"));
        }
    });
}
