mod application;
mod domain;
mod platform;

use application::{
    JsonSettingsRepository, KeychainSessionRepository, RemoteTransformer, SelectionCoordinator,
    SessionRepository, StoredSession,
};
use domain::{
    AppSettings, SelectionEvent, SettingsRepository, TransformOperation, TransformPreferences,
    TransformRequest, TransformResult, VerbalixError,
};
use platform::{MacAccessibility, TauriOverlay};
use std::{
    sync::Arc,
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
}

#[tauri::command]
fn accessibility_status(
    runtime: State<'_, Arc<AppRuntime>>,
    prompt: Option<bool>,
) -> bool {
    use application::SelectionPort;
    runtime.selection.permission_granted(prompt.unwrap_or(false))
}

#[tauri::command]
fn load_settings(runtime: State<'_, Arc<AppRuntime>>) -> Result<AppSettings, VerbalixError> {
    runtime.settings.load()
}

#[tauri::command]
fn save_settings(
    runtime: State<'_, Arc<AppRuntime>>,
    settings: AppSettings,
) -> Result<(), VerbalixError> {
    runtime.settings.save(&settings)
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
fn current_selection(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Option<domain::SelectionSnapshot> {
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
    let session = runtime
        .session
        .load()?
        .ok_or(VerbalixError::Unauthenticated)?;
    let request = TransformRequest {
        request_id: uuid::Uuid::new_v4(),
        operation,
        text: snapshot.text,
        preferences,
    };
    runtime
        .coordinator
        .transform(request, &session.access_token)
        .await
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

fn start_selection_observer(runtime: Arc<AppRuntime>) {
    thread::spawn(move || {
        let mut candidate_id = None;
        loop {
            let settings = runtime.settings.load().unwrap_or_default();
            if settings.automatic_toolbar {
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
                    }
                    _ => {}
                }
            }
            thread::sleep(Duration::from_millis(120));
        }
    });
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
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let config_dir = app.path().app_config_dir()?;
            let settings = Arc::new(JsonSettingsRepository::new(config_dir.join("settings.json")));
            let selection = Arc::new(MacAccessibility::new());
            let overlay = Arc::new(TauriOverlay::new(app.handle().clone()));
            let supabase_url = std::env::var("VERBALIX_SUPABASE_URL").unwrap_or_default();
            let endpoint = format!(
                "{}/functions/v1/transform",
                supabase_url.trim_end_matches('/')
            );
            let anonymous_key = std::env::var("VERBALIX_SUPABASE_ANON_KEY").unwrap_or_default();
            let provider = Arc::new(RemoteTransformer::new(endpoint, anonymous_key));
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
            });
            app.manage(runtime.clone());
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
            undo_replacement,
            dismiss_overlays
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Verbalix");
}
