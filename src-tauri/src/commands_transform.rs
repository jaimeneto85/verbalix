use crate::{
    application::{AiReadinessStatus, PublicationGuard, SessionRepository},
    commands::{current_ai_readiness, route_refresh_failure},
    domain::{
        SettingsRepository, TransformOperation, TransformPreferences, TransformRequest,
        TransformResult, VerbalixError,
    },
    AppRuntime,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

pub(crate) fn show_transform_failure(
    runtime: &AppRuntime,
    bounds: crate::domain::Rect,
    error: &VerbalixError,
    guard: PublicationGuard,
) {
    let message = match error {
        VerbalixError::PermissionDenied => "Permita o Acesso à Acessibilidade para continuar.",
        VerbalixError::SelectionUnavailable | VerbalixError::StaleSelection => {
            "A seleção mudou. Selecione o texto novamente."
        }
        VerbalixError::ProtectedField => "Este campo protegido não pode ser alterado.",
        VerbalixError::OperationInProgress => "A transformação anterior ainda está em andamento.",
        VerbalixError::TextTooLong => "A seleção ultrapassa o limite de 12.000 caracteres.",
        VerbalixError::Unauthenticated
        | VerbalixError::ProviderNotConfigured
        | VerbalixError::ProviderTimeout
        | VerbalixError::ProviderRejected
        | VerbalixError::InvalidResponse => {
            "O serviço de IA está indisponível. Tente novamente ou abra o Verbalix."
        }
        #[cfg(not(target_os = "macos"))]
        VerbalixError::UnsupportedPlatform => "Esta operação não está disponível nesta plataforma.",
        VerbalixError::LocalFailure => "Não foi possível aplicar o resultado. Tente novamente.",
    };
    let _ = runtime.overlay.show_error_guarded(bounds, message, guard);
}

#[tauri::command]
pub(crate) async fn transform_selection(
    app: AppHandle,
    runtime: State<'_, Arc<AppRuntime>>,
    operation: TransformOperation,
    preferences: Option<TransformPreferences>,
) -> Result<TransformResult, VerbalixError> {
    let snapshot = runtime
        .coordinator
        .current_snapshot()
        .ok_or(VerbalixError::SelectionUnavailable)?;
    let request_id = uuid::Uuid::new_v4();
    let snapshot = match runtime.coordinator.begin_transform(snapshot.id, request_id) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if let Ok(Some(guard)) = runtime.coordinator.feedback_guard(snapshot.id) {
                show_transform_failure(&runtime, snapshot.bounds, &error, guard);
            }
            return Err(error);
        }
    };
    let request = TransformRequest {
        request_id,
        operation,
        text: snapshot.text.clone(),
        preferences,
    };
    let result = transform_pinned(&app, &runtime, &snapshot, &request).await;
    if let Err(error) = &result {
        let _ = runtime.coordinator.abort_transform(request.request_id);
        if let Ok(Some(guard)) = runtime
            .coordinator
            .publication_guard(snapshot.id, request.request_id)
        {
            show_transform_failure(&runtime, snapshot.bounds, error, guard);
        }
    }
    result
}

async fn transform_pinned(
    app: &AppHandle,
    runtime: &AppRuntime,
    snapshot: &crate::domain::SelectionSnapshot,
    request: &TransformRequest,
) -> Result<TransformResult, VerbalixError> {
    let readiness = current_ai_readiness(runtime)?;
    if readiness.status != AiReadinessStatus::Ready {
        if readiness.status == AiReadinessStatus::LoginRequired {
            if runtime
                .coordinator
                .publication_guard(snapshot.id, request.request_id)?
                .is_some_and(|guard| guard.may_publish())
            {
                crate::show_main_window(app, "login_required");
            }
            return Err(VerbalixError::Unauthenticated);
        }
        return Err(VerbalixError::ProviderNotConfigured);
    }
    let stored = runtime
        .session
        .load()?
        .ok_or(VerbalixError::Unauthenticated)?;
    let session = match runtime.auth.refresh(&stored).await {
        Ok(session) => session,
        Err(error) => {
            let guard = runtime
                .coordinator
                .publication_guard(snapshot.id, request.request_id)
                .ok()
                .flatten();
            route_refresh_failure(
                &error,
                || {
                    if guard.as_ref().is_some_and(|guard| guard.may_publish()) {
                        crate::show_main_window(app, "login_required");
                    }
                },
                || {},
            );
            return Err(error);
        }
    };
    runtime.session.save(&session)?;
    let settings = runtime.settings.load()?;
    let response = runtime
        .coordinator
        .transform(
            snapshot.id,
            request.clone(),
            &session.access_token,
            settings.confirm_before_replace,
        )
        .await?;
    if settings.history_enabled {
        persist_history(runtime, request, &response, &session.access_token);
    }
    Ok(response)
}

fn persist_history(
    runtime: &AppRuntime,
    request: &TransformRequest,
    response: &TransformResult,
    access_token: &str,
) {
    let history = runtime.history.clone();
    let request = request.clone();
    let response = response.clone();
    let access_token = access_token.to_owned();
    tauri::async_runtime::spawn(async move {
        match tokio::time::timeout(
            std::time::Duration::from_secs(8),
            history.insert(&request, &response, &access_token),
        )
        .await
        {
            Ok(Ok(())) => crate::diagnostics::history::record("inserted", None),
            Ok(Err(error)) => crate::diagnostics::history::record("insert_failed", Some(&error)),
            Err(_) => crate::diagnostics::history::record(
                "insert_timeout",
                Some(&VerbalixError::ProviderTimeout),
            ),
        }
    });
}
