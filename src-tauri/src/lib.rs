mod application;
mod commands;
mod diagnostics;
mod domain;
mod platform;

use application::{
    JsonSettingsRepository, KeychainSessionRepository, RemoteAuthRepository,
    RemoteHistoryRepository, RemoteTransformer, RuntimePause, SelectionCoordinator,
};
use commands::*;
use domain::{SelectionEvent, SettingsRepository, VerbalixError};
use platform::{install_mouse_dismiss_monitor, MacAccessibility, SystemClipboard, TauriOverlay};
use std::{sync::Arc, thread, time::Duration};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

pub(crate) struct AppRuntime {
    pub coordinator: Arc<SelectionCoordinator>,
    pub overlay: Arc<TauriOverlay>,
    pub selection: Arc<MacAccessibility>,
    pub settings: Arc<JsonSettingsRepository>,
    pub session: Arc<KeychainSessionRepository>,
    pub clipboard: Arc<SystemClipboard>,
    pub history: Arc<RemoteHistoryRepository>,
    pub auth: Arc<RemoteAuthRepository>,
    pub pause: RuntimePause,
}

fn start_selection_observer(runtime: Arc<AppRuntime>) {
    thread::spawn(move || {
        let mut candidate_id = None;
        loop {
            let settings = runtime.settings.load().unwrap_or_default();
            let result = runtime.pause.run_polling(settings.automatic_toolbar, || {
                diagnostics::detection("polling");
                match runtime.coordinator.refresh_selection() {
                    Ok(Some(snapshot)) if candidate_id != Some(snapshot.id) => {
                        candidate_id = Some(snapshot.id);
                        thread::sleep(Duration::from_millis(150));
                        if !runtime.pause.is_paused() {
                            let _ = runtime
                                .coordinator
                                .dispatch(SelectionEvent::DebounceElapsed(snapshot.id));
                        }
                    }
                    Err(error @ VerbalixError::SelectionUnavailable)
                    | Err(error @ VerbalixError::ProtectedField)
                    | Err(error @ VerbalixError::PermissionDenied) => {
                        diagnostics::capture_failure("polling", &error);
                        candidate_id = None;
                        let _ = runtime.coordinator.dispatch(SelectionEvent::Invalidated);
                    }
                    _ => {}
                }
            });
            if result.is_none() {
                candidate_id = None;
            }
            thread::sleep(Duration::from_millis(120));
        }
    });
}

fn trigger_shortcut(runtime: &AppRuntime) {
    let _ = runtime
        .pause
        .run_global_shortcut(|| trigger_active_shortcut(runtime));
}

fn trigger_active_shortcut(runtime: &AppRuntime) {
    diagnostics::detection("shortcut");
    match runtime.coordinator.refresh_selection() {
        Ok(Some(snapshot)) => {
            let _ = runtime
                .coordinator
                .dispatch(SelectionEvent::DebounceElapsed(snapshot.id));
        }
        Err(VerbalixError::SelectionUnavailable) => {
            diagnostics::capture_failure("shortcut", &VerbalixError::SelectionUnavailable);
            use domain::{Rect, SelectionSnapshot, TextRange};
            let copied = runtime
                .pause
                .run_clipboard_fallback(|| runtime.clipboard.copy_selection_preserving_clipboard());
            if let Some(Ok(text)) = copied {
                let snapshot = SelectionSnapshot::new(
                    0,
                    "clipboard-fallback".to_owned(),
                    text.clone(),
                    TextRange {
                        location: 0,
                        length: text.encode_utf16().count() as i64,
                    },
                    Rect {
                        x: 24.0,
                        y: 80.0,
                        width: 1.0,
                        height: 1.0,
                    },
                    false,
                );
                let id = snapshot.id;
                let _ = runtime
                    .coordinator
                    .dispatch(SelectionEvent::Candidate(snapshot));
                let _ = runtime
                    .coordinator
                    .dispatch(SelectionEvent::DebounceElapsed(id));
            }
        }
        Err(error) => diagnostics::capture_failure("shortcut", &error),
        _ => {}
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "Configurações", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pausar", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings, &pause, &quit])?;
    let pause_item = pause.clone();
    TrayIconBuilder::new()
        .tooltip("Verbalix")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => app.exit(0),
            "pause" => {
                let runtime = app.state::<Arc<AppRuntime>>();
                let paused = runtime.pause.toggle();
                let _ = pause_item.set_text(if paused { "Retomar" } else { "Pausar" });
                if paused {
                    let _ = runtime.coordinator.dispatch(SelectionEvent::Invalidated);
                }
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub(crate) fn normalized_shortcut(shortcut: &str) -> String {
    shortcut.replace("Option", "Alt")
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let config_dir = app.path().app_config_dir()?;
            let settings = Arc::new(JsonSettingsRepository::new(
                config_dir.join("settings.json"),
            ));
            let selection = Arc::new(MacAccessibility::new());
            let overlay = Arc::new(TauriOverlay::new(app.handle().clone()));
            let supabase_url = std::env::var("VERBALIX_SUPABASE_URL").unwrap_or_default();
            let endpoint = format!(
                "{}/functions/v1/transform",
                supabase_url.trim_end_matches('/')
            );
            let anonymous_key = std::env::var("VERBALIX_SUPABASE_ANON_KEY").unwrap_or_default();
            let provider = Arc::new(RemoteTransformer::new(endpoint, anonymous_key.clone()));
            let coordinator = Arc::new(SelectionCoordinator::new(
                selection.clone(),
                overlay.clone(),
                provider,
            ));
            let runtime = Arc::new(AppRuntime {
                coordinator,
                overlay,
                selection,
                settings,
                session: Arc::new(KeychainSessionRepository::new(
                    "com.verbalix.desktop",
                    "supabase-access-token",
                )),
                clipboard: Arc::new(SystemClipboard::new().map_err(|error| {
                    let boxed: Box<dyn std::error::Error> = Box::new(error);
                    tauri::Error::Setup(boxed.into())
                })?),
                history: Arc::new(RemoteHistoryRepository::new(
                    supabase_url.clone(),
                    anonymous_key.clone(),
                )),
                auth: Arc::new(RemoteAuthRepository::new(supabase_url, anonymous_key)),
                pause: RuntimePause::default(),
            });
            app.manage(runtime.clone());
            let dismiss_runtime = runtime.clone();
            install_mouse_dismiss_monitor(Arc::new(move || {
                diagnostics::detection("mouse_dismiss");
                let _ = dismiss_runtime
                    .coordinator
                    .dispatch(SelectionEvent::Invalidated);
            }));
            let observer_runtime = runtime.clone();
            runtime.selection.start_observer(Arc::new(move || {
                let _ = observer_runtime.pause.run_ax_observer(|| {
                    diagnostics::detection("ax_observer");
                    match observer_runtime.coordinator.refresh_selection() {
                        Ok(Some(snapshot)) => {
                            thread::sleep(Duration::from_millis(150));
                            if !observer_runtime.pause.is_paused() {
                                let _ = observer_runtime
                                    .coordinator
                                    .dispatch(SelectionEvent::DebounceElapsed(snapshot.id));
                            }
                        }
                        Err(error) => {
                            diagnostics::capture_failure("ax_observer", &error);
                            let _ = observer_runtime
                                .coordinator
                                .dispatch(SelectionEvent::Invalidated);
                        }
                        _ => {}
                    }
                });
            }));
            let shortcut_runtime = runtime.clone();
            let shortcut = normalized_shortcut(&runtime.settings.load()?.shortcut);
            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_shortcuts([shortcut.as_str()])?
                    .with_handler(move |_app, _shortcut, event| {
                        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                            trigger_shortcut(&shortcut_runtime);
                        }
                    })
                    .build(),
            )?;
            start_selection_observer(runtime);
            setup_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            accessibility_status,
            load_settings,
            save_settings,
            save_session,
            has_session,
            clear_session,
            current_selection,
            current_note_result,
            refresh_selection,
            transform_selection,
            apply_preview,
            undo_replacement,
            dismiss_overlays,
            list_history,
            delete_history
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Verbalix");
}
