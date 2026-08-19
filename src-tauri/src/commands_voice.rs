use crate::{
    application::SessionRepository,
    domain::{MicrophonePermission, SettingsRepository, VerbalixError, VoiceProfileView},
    AppRuntime,
};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

#[tauri::command]
pub(crate) fn microphone_permission_status(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<MicrophonePermission, VerbalixError> {
    Ok(runtime.audio_capture.permission_status())
}

#[tauri::command]
pub(crate) async fn request_microphone_permission(
    app: AppHandle,
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<MicrophonePermission, VerbalixError> {
    let current = runtime.audio_capture.permission_status();
    if current != MicrophonePermission::NotDetermined {
        let _ = app.emit("microphone-permission", current);
        return Ok(current);
    }
    let app_clone = app.clone();
    crate::platform::request_microphone_permission(move |status| {
        let _ = app_clone.emit("microphone-permission", status);
    });
    Ok(MicrophonePermission::NotDetermined)
}

#[tauri::command]
pub(crate) fn begin_voice_enrollment(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<(), VerbalixError> {
    runtime.enrollment_session.begin()?;
    runtime.audio_capture.start().inspect_err(|_| {
        runtime.enrollment_session.cancel();
    })
}

#[tauri::command]
pub(crate) fn cancel_voice_enrollment(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<(), VerbalixError> {
    runtime.audio_capture.cancel();
    runtime.enrollment_session.cancel();
    Ok(())
}

#[tauri::command]
pub(crate) async fn finish_voice_enrollment(
    runtime: State<'_, Arc<AppRuntime>>,
    display_name: String,
) -> Result<VoiceProfileView, VerbalixError> {
    let sample = runtime.audio_capture.stop()?;
    runtime.enrollment_session.finish(sample)?;
    let sample = runtime.enrollment_session.take_sample()?;

    let session = runtime
        .session
        .load()?
        .ok_or(VerbalixError::Unauthenticated)?;
    let request_id = Uuid::new_v4();

    let view = runtime
        .voice_enrollment
        .enroll(&sample, &display_name, request_id, &session.access_token)
        .await
        .map_err(|_| VerbalixError::EnrollmentFailed)?;

    let mut settings = runtime.settings.load()?;
    settings.voice_profile_id = Some(view.voice_profile_id);
    runtime.settings.save(&settings)?;

    Ok(view)
}

#[tauri::command]
pub(crate) async fn delete_voice_profile(
    runtime: State<'_, Arc<AppRuntime>>,
    voice_profile_id: String,
) -> Result<(), VerbalixError> {
    let id = Uuid::parse_str(&voice_profile_id).map_err(|_| VerbalixError::LocalFailure)?;
    let session = runtime
        .session
        .load()?
        .ok_or(VerbalixError::Unauthenticated)?;

    runtime
        .voice_enrollment
        .delete(id, &session.access_token)
        .await?;

    let mut settings = runtime.settings.load()?;
    if settings.voice_profile_id == Some(id) {
        settings.voice_profile_id = None;
        runtime.settings.save(&settings)?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn voice_profile_status(
    runtime: State<'_, Arc<AppRuntime>>,
    voice_profile_id: String,
) -> Result<Option<VoiceProfileView>, VerbalixError> {
    let id = Uuid::parse_str(&voice_profile_id).map_err(|_| VerbalixError::LocalFailure)?;
    let session = runtime
        .session
        .load()?
        .ok_or(VerbalixError::Unauthenticated)?;
    runtime
        .voice_enrollment
        .status(id, &session.access_token)
        .await
}

#[tauri::command]
pub(crate) fn enrollment_level(runtime: State<'_, Arc<AppRuntime>>) -> Result<f32, VerbalixError> {
    Ok(runtime.audio_capture.level())
}
