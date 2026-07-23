use crate::{
    application::{HistoryItem, SessionRepository, StoredSession},
    domain::{
        AppSettings, SelectionEvent, SettingsRepository, TransformOperation, TransformPreferences,
        TransformRequest, TransformResult, VerbalixError,
    },
    normalized_shortcut, AppRuntime,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub(crate) fn accessibility_status(
    runtime: State<'_, Arc<AppRuntime>>,
    prompt: Option<bool>,
) -> bool {
    use crate::application::SelectionPort;
    runtime
        .selection
        .permission_granted(prompt.unwrap_or(false))
}

#[tauri::command]
pub(crate) fn load_settings(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<AppSettings, VerbalixError> {
    runtime.settings.load()
}

#[tauri::command]
pub(crate) fn save_settings(
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
pub(crate) fn refresh_selection(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<Option<crate::domain::SelectionSnapshot>, VerbalixError> {
    let snapshot = runtime.coordinator.refresh_selection()?;
    if let Some(snapshot) = &snapshot {
        runtime
            .coordinator
            .dispatch(SelectionEvent::DebounceElapsed(snapshot.id))?;
    }
    Ok(snapshot)
}

#[tauri::command]
pub(crate) async fn transform_selection(
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
    let preview = runtime.settings.load()?.confirm_before_replace;
    let response = runtime
        .coordinator
        .transform(request.clone(), &session.access_token, preview)
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
pub(crate) fn apply_preview(
    runtime: State<'_, Arc<AppRuntime>>,
    request_id: uuid::Uuid,
) -> Result<String, VerbalixError> {
    runtime.coordinator.apply_preview(request_id)
}

#[tauri::command]
pub(crate) fn undo_replacement(
    runtime: State<'_, Arc<AppRuntime>>,
    transformed_text: String,
) -> Result<(), VerbalixError> {
    runtime.coordinator.undo(&transformed_text)
}

#[tauri::command]
pub(crate) fn dismiss_overlays(runtime: State<'_, Arc<AppRuntime>>) -> Result<(), VerbalixError> {
    runtime.coordinator.dispatch(SelectionEvent::Invalidated)
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
