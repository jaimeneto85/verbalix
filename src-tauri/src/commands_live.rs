use crate::{
    application::{live_interpretation::LiveEventPayload, LiveEventFn, SessionRepository},
    domain::{LiveState, SettingsRepository, VerbalixError},
    AppRuntime,
};
use serde::Serialize;
use std::sync::Arc;
use tauri::{Emitter, State};
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveStatusResponse {
    status: String,
}

#[tauri::command]
pub(crate) async fn enter_live(
    runtime: State<'_, Arc<AppRuntime>>,
    target_language: String,
    voice_profile_id: String,
) -> Result<(), VerbalixError> {
    let stored = runtime
        .session
        .load()?
        .ok_or(VerbalixError::Unauthenticated)?;
    let token = stored.access_token;
    let profile_id =
        Uuid::parse_str(&voice_profile_id).map_err(|_| VerbalixError::VoiceProfileMissing)?;
    let route_to_virtual_mic = runtime
        .settings
        .load()
        .map(|s| s.output_to_virtual_mic)
        .unwrap_or(false);
    let guard = runtime.live_coordinator.enter_live(
        &target_language,
        profile_id,
        token,
        route_to_virtual_mic,
    )?;
    *runtime.on_air_guard.lock().unwrap() = Some(guard);
    Ok(())
}

#[tauri::command]
pub(crate) fn leave_live(runtime: State<'_, Arc<AppRuntime>>) -> Result<(), VerbalixError> {
    runtime.live_coordinator.leave_live();
    *runtime.on_air_guard.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub(crate) fn live_status(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<LiveStatusResponse, VerbalixError> {
    let status = match runtime.live_coordinator.live_state() {
        LiveState::Idle => "idle",
        LiveState::Preparing => "listening",
        LiveState::OnAir => "listening",
        LiveState::Recovering => "processing",
        LiveState::Stopping => "idle",
        LiveState::Failed => "error",
    };
    let _vmic_status = runtime.virtual_mic_status();
    let _vmic_metrics = runtime.live_coordinator.vmic_metrics();
    Ok(LiveStatusResponse {
        status: status.to_owned(),
    })
}

#[tauri::command]
pub(crate) fn set_target_language(
    runtime: State<'_, Arc<AppRuntime>>,
    language: String,
) -> Result<(), VerbalixError> {
    let mut settings = runtime.settings.load()?;
    settings.target_language = language;
    runtime.settings.save(&settings.validate()?)
}

pub(crate) fn make_live_emitter(app: tauri::AppHandle) -> LiveEventFn {
    Arc::new(move |payload: LiveEventPayload| {
        let mut map = serde_json::json!({ "status": payload.status });
        if let Some(ms) = payload.stage_ms {
            map["stageMs"] =
                serde_json::json!({ "stt": ms.stt, "translate": ms.translate, "tts": ms.tts });
        }
        if let Some(sid) = payload.segment_id {
            map["segmentId"] = serde_json::json!(sid);
        }
        if let Some(lang) = payload.detected_language {
            map["detectedLanguage"] = serde_json::json!(lang);
        }
        if let Some(fam) = payload.first_audio_ms {
            map["firstAudioMs"] = serde_json::json!(fam);
        }
        if let Some(tl) = payload.target_language {
            map["targetLanguage"] = serde_json::json!(tl);
        }
        let _ = app.emit("live-state", map);
    })
}
