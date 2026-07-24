use crate::{
    application::{AiReadinessStatus, PublicationGuard, SessionRepository},
    commands::{
        current_ai_readiness, route_refresh_failure, show_provider_unavailable, show_readiness,
    },
    domain::{
        SettingsRepository, TransformOperation, TransformPreferences, TransformRequest,
        TransformResult, VerbalixError,
    },
    AppRuntime,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

fn show_transform_failure(
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
    let request = TransformRequest {
        request_id: uuid::Uuid::new_v4(),
        operation,
        text: snapshot.text.clone(),
        preferences,
    };
    runtime
        .coordinator
        .begin_transform(snapshot.id, request.request_id)?;
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
        let _ = runtime
            .history
            .insert(request, &response, &session.access_token)
            .await;
    }
    Ok(response)
}
