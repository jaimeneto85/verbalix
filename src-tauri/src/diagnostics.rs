use crate::application::VirtualMicStatus;
use crate::domain::{SelectionSnapshot, VerbalixError};
#[cfg(target_os = "macos")]
use crate::platform::macos_focus::{AxCategory, AxStage, ExtractionOrigin};
#[cfg(target_os = "macos")]
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

mod latency;
#[cfg(test)]
pub(crate) use latency::LiveLatencyAggregator;
pub(crate) use latency::{emit_latency_summary, increment_underruns, record_latency, LatencyStage};

pub(crate) mod history;

pub(crate) fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("VERBALIX_DIAGNOSTICS")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE"))
            .unwrap_or(false)
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn ax_resolution(stage: AxStage, origin: ExtractionOrigin, category: AxCategory) {
    static LAST: OnceLock<Mutex<HashMap<(AxStage, ExtractionOrigin), AxCategory>>> =
        OnceLock::new();
    let key = (stage, origin);
    let should_emit = LAST
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map(|mut last| should_emit_ax_transition(&mut last, key, category))
        .unwrap_or(false);
    if should_emit {
        emit(
            "ax_resolution",
            "status",
            &format!(
                "stage={} origin={} category={}",
                stage.as_str(),
                origin.as_str(),
                category.as_str()
            ),
        );
    }
}

#[cfg(target_os = "macos")]
fn should_emit_ax_transition(
    last: &mut HashMap<(AxStage, ExtractionOrigin), AxCategory>,
    key: (AxStage, ExtractionOrigin),
    category: AxCategory,
) -> bool {
    last.insert(key, category) != Some(category)
}

fn emit(stage: &str, event: &str, metadata: &str) {
    if enabled() {
        eprintln!("[verbalix:{stage}] {event} {metadata}");
    }
}

pub fn detection(origin: &str) {
    emit("detection", "triggered", &format!("origin={origin}"));
}

pub fn lifecycle(event: &'static str, origin: &'static str) {
    emit("lifecycle", event, &lifecycle_metadata(origin));
}

pub fn accessibility(trusted: bool) {
    let status = if trusted {
        "trusted=true"
    } else {
        "trusted=false"
    };
    emit("accessibility", "status", status);
}

pub fn ai_readiness(status: &str) {
    emit("ai", "readiness", &format!("status={status}"));
}

pub fn virtual_mic(status: VirtualMicStatus, buffer_depth: u32, underruns: u64) {
    emit(
        "virtual_mic",
        "status",
        &virtual_mic_metadata(status, buffer_depth, underruns),
    );
}

fn virtual_mic_metadata(status: VirtualMicStatus, buffer_depth: u32, underruns: u64) -> String {
    let label = match status {
        VirtualMicStatus::NotInstalled => "not_installed",
        VirtualMicStatus::Installed => "installed",
        VirtualMicStatus::IncompatibleVersion => "incompatible_version",
    };
    format!("status={label} buffer_depth={buffer_depth} underruns={underruns}")
}

pub fn capture_success(snapshot: &SelectionSnapshot) {
    emit("capture", "success", &snapshot_metadata(snapshot));
}

pub fn capture_failure(origin: &str, error: &VerbalixError) {
    emit(
        "capture",
        "failure",
        &format!("origin={origin} error={}", error_code(error)),
    );
}

pub fn coordinator(decision: &str, snapshot: Option<&SelectionSnapshot>) {
    let metadata = snapshot
        .map(snapshot_metadata)
        .unwrap_or_else(|| "snapshot_id=none".to_owned());
    emit("coordinator", decision, &metadata);
}

pub fn overlay(stage: &str, label: &str, sequence: u64) {
    emit(
        "overlay",
        stage,
        &format!("label={label} sequence={sequence}"),
    );
}

pub fn overlay_visibility(label: &str, sequence: u64, visible: bool) {
    emit(
        "overlay",
        "visibility",
        &format!("label={label} sequence={sequence} visible={visible}"),
    );
}

pub fn overlay_position(label: &str, sequence: u64, x: f64, y: f64) {
    emit(
        "overlay",
        "positioned",
        &format!("label={label} sequence={sequence} x={x:.1} y={y:.1}"),
    );
}

fn snapshot_metadata(snapshot: &SelectionSnapshot) -> String {
    format!(
        "snapshot_id={} geometry_source={} extraction_strategy={} writable={}",
        snapshot.id,
        snapshot
            .geometry_source
            .map(|source| source.as_str())
            .unwrap_or("unknown"),
        snapshot.extraction_strategy.as_str(),
        snapshot.writable
    )
}

fn lifecycle_metadata(origin: &'static str) -> String {
    format!("origin={origin}")
}

pub(super) fn error_code(error: &VerbalixError) -> &'static str {
    match error {
        VerbalixError::PermissionDenied => "permission_denied",
        VerbalixError::SelectionUnavailable => "selection_unavailable",
        VerbalixError::ProtectedField => "protected_field",
        VerbalixError::StaleSelection => "stale_selection",
        VerbalixError::OperationInProgress => "operation_in_progress",
        VerbalixError::TextTooLong => "text_too_long",
        VerbalixError::Unauthenticated => "unauthenticated",
        VerbalixError::ProviderNotConfigured => "provider_not_configured",
        VerbalixError::ProviderTimeout => "provider_timeout",
        VerbalixError::ProviderRejected => "provider_rejected",
        VerbalixError::InvalidResponse => "invalid_response",
        VerbalixError::MicrophonePermissionDenied => "microphone_permission_denied",
        VerbalixError::AudioCaptureFailed => "audio_capture_failed",
        VerbalixError::EnrollmentFailed => "enrollment_failed",
        #[cfg(not(target_os = "macos"))]
        VerbalixError::UnsupportedPlatform => "unsupported_platform",
        VerbalixError::LocalFailure => "local_failure",
        VerbalixError::LiveSessionInactive => "live_session_inactive",
        VerbalixError::TargetLanguageUnsupported => "target_language_unsupported",
        VerbalixError::VoiceProfileMissing => "voice_profile_missing",
        VerbalixError::AudioPlaybackFailed => "audio_playback_failed",
        VerbalixError::InterpretationFailed => "interpretation_failed",
        VerbalixError::SttFailed => "stt_failed",
        VerbalixError::TranslationFailed => "translation_failed",
        VerbalixError::TtsFailed => "tts_failed",
        VerbalixError::VirtualMicUnavailable => "virtual_mic_unavailable",
        VerbalixError::VirtualMicSelectedAsInput => "virtual_mic_selected_as_input",
    }
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
