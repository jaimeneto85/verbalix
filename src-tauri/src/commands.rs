use crate::{
    application::{
        classify_refresh_failure, epoch_secs_now, evaluate_ai_readiness, merge_preferences,
        AiReadiness, AiReadinessStatus, HistoryItem, PublicBackendConfig, RefreshFailureRoute,
        RemotePreferencesRepository, SessionRepository, StoredSession,
    },
    domain::{AppSettings, SelectionEvent, SettingsRepository, VerbalixError},
    AppRuntime,
};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

pub(crate) fn normalized_shortcut(shortcut: &str) -> String {
    shortcut.replace("Option", "Alt")
}

pub(crate) fn current_ai_readiness(runtime: &AppRuntime) -> Result<AiReadiness, VerbalixError> {
    if !runtime.backend_config.configured {
        return Ok(evaluate_ai_readiness(false, false));
    }
    let has_session = runtime.session.load()?.is_some();
    Ok(evaluate_ai_readiness(true, has_session))
}

pub(crate) fn show_readiness(runtime: &AppRuntime, readiness: &AiReadiness) {
    crate::diagnostics::ai_readiness(readiness.status.as_str());
    if readiness.status == AiReadinessStatus::Ready {
        return;
    }
    if let Some(snapshot) = runtime.coordinator.current_snapshot() {
        let _ = runtime
            .overlay
            .show_error(snapshot.bounds, readiness.message);
    }
}

pub(crate) fn show_provider_unavailable(runtime: &AppRuntime) {
    crate::diagnostics::ai_readiness("provider_unavailable");
    if let Some(snapshot) = runtime.coordinator.current_snapshot() {
        let _ = runtime.overlay.show_error(
            snapshot.bounds,
            "O serviço de IA está indisponível. Tente novamente ou abra o Verbalix.",
        );
    }
}

pub(crate) fn route_refresh_failure(
    error: &VerbalixError,
    on_login_required: impl FnOnce(),
    on_provider_unavailable: impl FnOnce(),
) {
    match classify_refresh_failure(error) {
        RefreshFailureRoute::LoginRequired => on_login_required(),
        RefreshFailureRoute::ProviderUnavailable => on_provider_unavailable(),
    }
}

#[tauri::command]
pub(crate) fn public_backend_config(runtime: State<'_, Arc<AppRuntime>>) -> PublicBackendConfig {
    runtime.backend_config.clone()
}

#[tauri::command]
pub(crate) fn ai_readiness(
    app: AppHandle,
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<AiReadiness, VerbalixError> {
    let readiness = current_ai_readiness(&runtime).inspect_err(|_| {
        show_provider_unavailable(&runtime);
    })?;
    show_readiness(&runtime, &readiness);
    if readiness.status == AiReadinessStatus::LoginRequired {
        crate::show_main_window(&app, "login_required");
    }
    Ok(readiness)
}

#[tauri::command]
pub(crate) fn open_main_window(app: AppHandle) {
    crate::show_main_window(&app, "ai_action");
}

#[tauri::command]
pub(crate) fn accessibility_status(
    runtime: State<'_, Arc<AppRuntime>>,
    prompt: Option<bool>,
) -> bool {
    use crate::application::SelectionPort;
    let trusted = runtime
        .selection
        .permission_granted(prompt.unwrap_or(false));
    crate::diagnostics::accessibility(trusted);
    trusted
}

#[tauri::command]
pub(crate) fn load_settings(
    app: AppHandle,
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<AppSettings, VerbalixError> {
    let local = runtime.settings.load()?;

    let Some(repo) = runtime.remote_preferences.as_ref().map(Arc::clone) else {
        return Ok(local);
    };
    let session = match runtime.session.load()? {
        Some(s) => s,
        None => return Ok(local),
    };

    let runtime_clone = Arc::clone(&*runtime);
    tauri::async_runtime::spawn(background_sync(
        app,
        runtime_clone,
        repo,
        session,
        local.clone(),
    ));

    Ok(local)
}

async fn background_sync(
    app: AppHandle,
    runtime: Arc<AppRuntime>,
    repo: Arc<RemotePreferencesRepository>,
    session: StoredSession,
    local: AppSettings,
) {
    let sync_store = &runtime.preferences_sync;

    let captured_seq = sync_store.load().map(|m| m.sequence).unwrap_or(0);

    let remote = match repo.fetch(&session.access_token).await {
        Ok(r) => r,
        Err(_) => return,
    };

    let current_meta = sync_store.load();
    let current_seq = current_meta.as_ref().map(|m| m.sequence).unwrap_or(0);
    if current_seq != captured_seq {
        return;
    }

    let outcome = merge_preferences(&local, current_meta.as_ref(), remote);

    let now_secs = epoch_secs_now();
    let _ = sync_store.record_synced(now_secs);

    if outcome.settings != local {
        let _ = runtime.settings.save(&outcome.settings);
    }

    if let Ok(mut guard) = runtime.synced_settings.lock() {
        *guard = Some(outcome.settings.clone());
    }
    let _ = app.emit("preferences-synced", &outcome.settings);

    if outcome.needs_push {
        let _ = repo.upsert(&outcome.settings, &session.access_token).await;
    }
}

#[tauri::command]
pub(crate) fn current_synced_preferences(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<Option<AppSettings>, VerbalixError> {
    runtime
        .synced_settings
        .lock()
        .map(|g| g.clone())
        .map_err(|_| VerbalixError::LocalFailure)
}

#[tauri::command]
pub(crate) fn save_settings(
    app: AppHandle,
    runtime: State<'_, Arc<AppRuntime>>,
    settings: AppSettings,
) -> Result<(), VerbalixError> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let now_secs = epoch_secs_now();
    let _ = runtime.preferences_sync.record_change(now_secs);

    runtime.settings.save(&settings)?;
    let shortcut = normalized_shortcut(&settings.shortcut);
    app.global_shortcut()
        .unregister_all()
        .map_err(|_| VerbalixError::LocalFailure)?;
    app.global_shortcut()
        .register(shortcut.as_str())
        .map_err(|_| VerbalixError::LocalFailure)?;
    if let (Some(repo), Ok(Some(session))) = (&runtime.remote_preferences, runtime.session.load()) {
        let repo = Arc::clone(repo);
        let settings = settings.clone();
        tauri::async_runtime::spawn(async move {
            let _ = repo.upsert(&settings, &session.access_token).await;
        });
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn save_session(
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
pub(crate) fn has_session(runtime: State<'_, Arc<AppRuntime>>) -> Result<bool, VerbalixError> {
    runtime.session.load().map(|session| session.is_some())
}

#[tauri::command]
pub(crate) fn clear_session(runtime: State<'_, Arc<AppRuntime>>) -> Result<(), VerbalixError> {
    runtime.session.clear()
}

#[tauri::command]
pub(crate) fn current_selection(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Option<crate::domain::SelectionSnapshot> {
    runtime.coordinator.current_snapshot()
}

#[tauri::command]
pub(crate) fn current_note_result(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<Option<crate::platform::NoteResultPayload>, VerbalixError> {
    runtime.overlay.current_note_result()
}

#[tauri::command]
pub(crate) fn refresh_selection(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<Option<crate::domain::SelectionSnapshot>, VerbalixError> {
    if runtime.pause.is_paused() {
        return Ok(None);
    }
    let snapshot = runtime.coordinator.refresh_selection()?;
    if let Some(snapshot) = &snapshot {
        runtime
            .coordinator
            .dispatch(SelectionEvent::DebounceElapsed(snapshot.id))?;
    }
    Ok(snapshot)
}

#[tauri::command]
pub(crate) fn apply_preview(
    runtime: State<'_, Arc<AppRuntime>>,
    request_id: uuid::Uuid,
) -> Result<String, VerbalixError> {
    let feedback = runtime.coordinator.preview_feedback(request_id)?;
    let result = runtime.coordinator.apply_preview(request_id);
    if let (Err(error), Some((bounds, guard))) = (&result, feedback) {
        crate::commands_transform::show_transform_failure(&runtime, bounds, error, guard);
    }
    result
}

#[tauri::command]
pub(crate) fn undo_replacement(
    runtime: State<'_, Arc<AppRuntime>>,
    transformed_text: String,
) -> Result<(), VerbalixError> {
    let feedback = runtime.coordinator.undo_feedback(&transformed_text)?;
    let result = runtime.coordinator.undo(&transformed_text);
    if let (Err(error), Some((bounds, guard))) = (&result, feedback) {
        crate::commands_transform::show_transform_failure(&runtime, bounds, error, guard);
    }
    result
}

#[tauri::command]
pub(crate) async fn list_history(
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
pub(crate) async fn delete_history(
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

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
