use crate::{
    application::{
        classify_refresh_failure, evaluate_ai_readiness, AiReadiness, AiReadinessStatus,
        HistoryItem, PublicBackendConfig, RefreshFailureRoute, RuntimePause, SelectionCoordinator,
        SessionRepository,
    },
    domain::{SelectionEvent, VerbalixError},
    AppRuntime,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

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

fn error_bounds_with(
    coordinator: &SelectionCoordinator,
    pause: &RuntimePause,
) -> Option<crate::domain::Rect> {
    coordinator
        .current_snapshot()
        .map(|s| s.bounds)
        .or_else(|| {
            pause
                .is_action_in_flight()
                .then(|| coordinator.last_known_bounds())
                .flatten()
        })
}

fn error_bounds(runtime: &AppRuntime) -> Option<crate::domain::Rect> {
    error_bounds_with(&runtime.coordinator, &runtime.pause)
}

pub(crate) fn show_readiness(runtime: &AppRuntime, readiness: &AiReadiness) {
    crate::diagnostics::ai_readiness(readiness.status.as_str());
    if readiness.status == AiReadinessStatus::Ready {
        return;
    }
    if let Some(bounds) = error_bounds(runtime) {
        let _ = runtime.overlay.show_error(bounds, readiness.message);
    }
}

pub(crate) fn show_provider_unavailable(runtime: &AppRuntime) {
    crate::diagnostics::ai_readiness("provider_unavailable");
    if let Some(bounds) = error_bounds(runtime) {
        let _ = runtime.overlay.show_error(
            bounds,
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
