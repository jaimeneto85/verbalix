use crate::{
    application::{
        evaluate_ai_readiness, AiReadiness, AiReadinessStatus, HistoryItem, PublicBackendConfig,
        SessionRepository, StoredSession,
    },
    domain::{
        AppSettings, SelectionEvent, SettingsRepository, TransformOperation, TransformPreferences,
        TransformRequest, TransformResult, VerbalixError,
    },
    normalized_shortcut, AppRuntime,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

fn current_ai_readiness(runtime: &AppRuntime) -> Result<AiReadiness, VerbalixError> {
    if !runtime.backend_config.configured {
        return Ok(evaluate_ai_readiness(false, false));
    }
    let has_session = runtime.session.load()?.is_some();
    Ok(evaluate_ai_readiness(true, has_session))
}

fn show_readiness(runtime: &AppRuntime, readiness: &AiReadiness) {
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

fn show_provider_unavailable(runtime: &AppRuntime) {
    crate::diagnostics::ai_readiness("provider_unavailable");
    if let Some(snapshot) = runtime.coordinator.current_snapshot() {
        let _ = runtime.overlay.show_error(
            snapshot.bounds,
            "O serviço de IA está indisponível. Tente novamente ou abra o Verbalix.",
        );
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
pub(crate) async fn transform_selection(
    app: AppHandle,
    runtime: State<'_, Arc<AppRuntime>>,
    operation: TransformOperation,
    preferences: Option<TransformPreferences>,
) -> Result<TransformResult, VerbalixError> {
    let readiness = current_ai_readiness(&runtime).inspect_err(|_| {
        show_provider_unavailable(&runtime);
    })?;
    if readiness.status != AiReadinessStatus::Ready {
        show_readiness(&runtime, &readiness);
        if readiness.status == AiReadinessStatus::LoginRequired {
            crate::show_main_window(&app, "login_required");
            return Err(VerbalixError::Unauthenticated);
        }
        return Err(VerbalixError::ProviderNotConfigured);
    }
    let snapshot = runtime
        .coordinator
        .current_snapshot()
        .ok_or(VerbalixError::SelectionUnavailable)?;
    let stored = runtime
        .session
        .load()?
        .ok_or(VerbalixError::Unauthenticated)?;
    let session = match runtime.auth.refresh(&stored).await {
        Ok(session) => session,
        Err(error) => {
            let readiness = AiReadiness::login_required();
            show_readiness(&runtime, &readiness);
            crate::show_main_window(&app, "login_required");
            return Err(error);
        }
    };
    runtime.session.save(&session).inspect_err(|_| {
        show_provider_unavailable(&runtime);
    })?;
    let request = TransformRequest {
        request_id: uuid::Uuid::new_v4(),
        operation,
        text: snapshot.text,
        preferences,
    };
    let preview = runtime.settings.load()?.confirm_before_replace;
    let response = match runtime
        .coordinator
        .transform(request.clone(), &session.access_token, preview)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            show_provider_unavailable(&runtime);
            return Err(error);
        }
    };
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
