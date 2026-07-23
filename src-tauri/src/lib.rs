mod application;
mod domain;
mod platform;

use application::{
    HistoryItem, JsonSettingsRepository, KeychainSessionRepository, RemoteAuthRepository,
    RemoteHistoryRepository, RemoteTransformer, SelectionCoordinator, SessionRepository,
    StoredSession,
};
use domain::{
    AppSettings, SelectionEvent, SettingsRepository, TransformOperation, TransformPreferences,
    TransformRequest, TransformResult, VerbalixError,
};
use platform::{MacAccessibility, SystemClipboard, TauriOverlay};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, State,
};

struct AppRuntime {
    coordinator: Arc<SelectionCoordinator>,
    selection: Arc<MacAccessibility>,
    settings: Arc<JsonSettingsRepository>,
    session: Arc<KeychainSessionRepository>,
    clipboard: Arc<SystemClipboard>,
    history: Arc<RemoteHistoryRepository>,
    auth: Arc<RemoteAuthRepository>,
    paused: AtomicBool,
}

#[tauri::command]
fn accessibility_status(runtime: State<'_, Arc<AppRuntime>>, prompt: Option<bool>) -> bool {
    use application::SelectionPort;
    runtime
        .selection
        .permission_granted(prompt.unwrap_or(false))
}

#[tauri::command]
fn load_settings(runtime: State<'_, Arc<AppRuntime>>) -> Result<AppSettings, VerbalixError> {
    runtime.settings.load()
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    runtime: State<'_, Arc<AppRuntime>>,
    settings: AppSettings,
) -> Result<(), VerbalixError> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    runtime.settings.save(&settings)?;
    let shortcut = normalized_shortcut(&settings.shortcut);
    app.global_shortcut()
        .unregister_all()
        .map_err(|_| VerbalixError::LocalFailure)?;
    app.global_shortcut()
        .register(shortcut.as_str())
        .map_err(|_| VerbalixError::LocalFailure)
}

#[tauri::command]
fn save_session(
    runtime: State<'_, Arc<AppRuntime>>,
    access_token: String,
    refresh_token: String,
) -> Result<(), VerbalixError> {
    runtime.session.save(&StoredSession {
        access_token,
        refresh_token,
    })
}

#[tauri::command]
fn has_session(runtime: State<'_, Arc<AppRuntime>>) -> Result<bool, VerbalixError> {
    runtime.session.load().map(|session| session.is_some())
}

#[tauri::command]
fn clear_session(runtime: State<'_, Arc<AppRuntime>>) -> Result<(), VerbalixError> {
    runtime.session.clear()
}

#[tauri::command]
fn current_selection(runtime: State<'_, Arc<AppRuntime>>) -> Option<domain::SelectionSnapshot> {
    runtime.coordinator.current_snapshot()
}

#[tauri::command]
fn refresh_selection(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<Option<domain::SelectionSnapshot>, VerbalixError> {
    let snapshot = runtime.coordinator.refresh_selection()?;
    if let Some(snapshot) = &snapshot {
        runtime
            .coordinator
            .dispatch(SelectionEvent::DebounceElapsed(snapshot.id))?;
    }
    Ok(snapshot)
}

#[tauri::command]
async fn transform_selection(
    runtime: State<'_, Arc<AppRuntime>>,
    operation: TransformOperation,
    preferences: Option<TransformPreferences>,
) -> Result<TransformResult, VerbalixError> {
    let snapshot = runtime
        .coordinator
        .current_snapshot()
        .ok_or(VerbalixError::SelectionUnavailable)?;
    let stored = runtime
        .session
        .load()?
        .ok_or(VerbalixError::Unauthenticated)?;
    let session = runtime.auth.refresh(&stored).await?;
    runtime.session.save(&session)?;
    let request = TransformRequest {
        request_id: uuid::Uuid::new_v4(),
        operation,
        text: snapshot.text,
        preferences,
    };
    let preview_writable = runtime.settings.load()?.confirm_before_replace;
    let response = runtime
        .coordinator
        .transform(request.clone(), &session.access_token, preview_writable)
        .await?;
    if runtime.settings.load()?.history_enabled {
        let _ = runtime
            .history
            .insert(&request, &response, &session.access_token)
            .await;
    }
    Ok(response)
}

#[tauri::command]
fn apply_preview(
    runtime: State<'_, Arc<AppRuntime>>,
    request_id: uuid::Uuid,
) -> Result<String, VerbalixError> {
    runtime.coordinator.apply_preview(request_id)
}

#[tauri::command]
fn undo_replacement(
    runtime: State<'_, Arc<AppRuntime>>,
    transformed_text: String,
) -> Result<(), VerbalixError> {
    runtime.coordinator.undo(&transformed_text)
}

#[tauri::command]
fn dismiss_overlays(runtime: State<'_, Arc<AppRuntime>>) -> Result<(), VerbalixError> {
    runtime.coordinator.dispatch(SelectionEvent::Invalidated)
}

#[tauri::command]
async fn list_history(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<Vec<HistoryItem>, VerbalixError> {
    let stored = runtime
        .session
        .load()?
        .ok_or(VerbalixError::Unauthenticated)?;
    let session = runtime.auth.refresh(&stored).await?;
    runtime.session.save(&session)?;
    runtime.history.list(&session.access_token).await
}

#[tauri::command]
async fn delete_history(
    runtime: State<'_, Arc<AppRuntime>>,
    id: Option<uuid::Uuid>,
) -> Result<(), VerbalixError> {
    let stored = runtime
        .session
        .load()?
        .ok_or(VerbalixError::Unauthenticated)?;
    let session = runtime.auth.refresh(&stored).await?;
    runtime.session.save(&session)?;
    runtime.history.delete(id, &session.access_token).await
}

fn start_selection_observer(runtime: Arc<AppRuntime>) {
    thread::spawn(move || {
        let mut candidate_id = None;
        loop {
            let settings = runtime.settings.load().unwrap_or_default();
            if settings.automatic_toolbar && !runtime.paused.load(Ordering::Relaxed) {
                match runtime.coordinator.refresh_selection() {
                    Ok(Some(snapshot)) if candidate_id != Some(snapshot.id) => {
                        candidate_id = Some(snapshot.id);
                        thread::sleep(Duration::from_millis(150));
                        let _ = runtime
                            .coordinator
                            .dispatch(SelectionEvent::DebounceElapsed(snapshot.id));
                    }
                    Err(VerbalixError::SelectionUnavailable)
                    | Err(VerbalixError::ProtectedField)
                    | Err(VerbalixError::PermissionDenied) => {
                        candidate_id = None;
                        let _ = runtime.coordinator.dispatch(SelectionEvent::Invalidated);
                    }
                    _ => {}
                }
            }
            if runtime.paused.load(Ordering::Relaxed) {
                candidate_id = None;
            }
            thread::sleep(Duration::from_millis(120));
        }
    });
}

fn trigger_shortcut(runtime: &AppRuntime) {
    match runtime.coordinator.refresh_selection() {
        Ok(Some(snapshot)) => {
            let _ = runtime
                .coordinator
                .dispatch(SelectionEvent::DebounceElapsed(snapshot.id));
        }
        Err(VerbalixError::SelectionUnavailable) => {
            use domain::{Rect, SelectionSnapshot, TextRange};
            if let Ok(text) = runtime.clipboard.copy_selection_preserving_clipboard() {
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
        _ => {}
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "Configurações", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pausar", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings, &pause, &quit])?;
    TrayIconBuilder::new()
        .tooltip("Verbalix")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => app.exit(0),
            "pause" => {
                let runtime = app.state::<Arc<AppRuntime>>();
                let paused = runtime.paused.load(Ordering::Relaxed);
                runtime.paused.store(!paused, Ordering::Relaxed);
                if !paused {
                    let _ = runtime.coordinator.dispatch(SelectionEvent::Invalidated);
                }
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

fn normalized_shortcut(shortcut: &str) -> String {
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
                overlay,
                provider,
            ));
            let runtime = Arc::new(AppRuntime {
                coordinator,
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
                paused: AtomicBool::new(false),
            });
            app.manage(runtime.clone());
            let observer_runtime = runtime.clone();
            runtime.selection.start_observer(Arc::new(move || {
                match observer_runtime.coordinator.refresh_selection() {
                    Ok(Some(snapshot)) => {
                        thread::sleep(Duration::from_millis(150));
                        let _ = observer_runtime
                            .coordinator
                            .dispatch(SelectionEvent::DebounceElapsed(snapshot.id));
                    }
                    _ => {
                        let _ = observer_runtime
                            .coordinator
                            .dispatch(SelectionEvent::Invalidated);
                    }
                }
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
