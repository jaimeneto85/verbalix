use crate::application::VirtualMicStatus;
use crate::domain::{SelectionSnapshot, VerbalixError};
#[cfg(target_os = "macos")]
use crate::platform::macos_focus::{AxCategory, AxStage, ExtractionOrigin};
#[cfg(target_os = "macos")]
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const BUCKET_BOUNDARIES_MS: [u64; 10] = [50, 100, 150, 200, 300, 500, 750, 1000, 2000, 5000];
const N_LATENCY_BUCKETS: usize = BUCKET_BOUNDARIES_MS.len() + 1;

#[derive(Copy, Clone)]
pub(crate) enum LatencyStage {
    CaptureToRequest,
    Ttfb,
    FirstAudio,
    PlaybackEnd,
}

pub(crate) struct LiveLatencyAggregator {
    capture_to_request: [u64; N_LATENCY_BUCKETS],
    ttfb: [u64; N_LATENCY_BUCKETS],
    first_audio: [u64; N_LATENCY_BUCKETS],
    playback_end: [u64; N_LATENCY_BUCKETS],
    pub(crate) underruns: u64,
}

impl LiveLatencyAggregator {
    pub(crate) const fn new() -> Self {
        Self {
            capture_to_request: [0; N_LATENCY_BUCKETS],
            ttfb: [0; N_LATENCY_BUCKETS],
            first_audio: [0; N_LATENCY_BUCKETS],
            playback_end: [0; N_LATENCY_BUCKETS],
            underruns: 0,
        }
    }

    pub(crate) fn record(&mut self, stage: LatencyStage, ms: u64) {
        let idx = BUCKET_BOUNDARIES_MS
            .iter()
            .position(|&b| ms <= b)
            .unwrap_or(N_LATENCY_BUCKETS - 1);
        self.buckets_mut(stage)[idx] += 1;
    }

    pub(crate) fn p50(&self, stage: LatencyStage) -> u64 {
        percentile_from_buckets(self.buckets(stage), 50)
    }

    pub(crate) fn p95(&self, stage: LatencyStage) -> u64 {
        percentile_from_buckets(self.buckets(stage), 95)
    }

    pub(crate) fn increment_underruns(&mut self) {
        self.underruns += 1;
    }

    fn buckets(&self, stage: LatencyStage) -> &[u64; N_LATENCY_BUCKETS] {
        match stage {
            LatencyStage::CaptureToRequest => &self.capture_to_request,
            LatencyStage::Ttfb => &self.ttfb,
            LatencyStage::FirstAudio => &self.first_audio,
            LatencyStage::PlaybackEnd => &self.playback_end,
        }
    }

    fn buckets_mut(&mut self, stage: LatencyStage) -> &mut [u64; N_LATENCY_BUCKETS] {
        match stage {
            LatencyStage::CaptureToRequest => &mut self.capture_to_request,
            LatencyStage::Ttfb => &mut self.ttfb,
            LatencyStage::FirstAudio => &mut self.first_audio,
            LatencyStage::PlaybackEnd => &mut self.playback_end,
        }
    }
}

fn percentile_from_buckets(buckets: &[u64; N_LATENCY_BUCKETS], pct: u64) -> u64 {
    let total: u64 = buckets.iter().sum();
    if total == 0 {
        return 0;
    }
    let target = (total * pct).div_ceil(100).max(1);
    let mut cumulative = 0u64;
    for (i, &count) in buckets.iter().enumerate() {
        cumulative += count;
        if cumulative >= target {
            return if i < BUCKET_BOUNDARIES_MS.len() {
                BUCKET_BOUNDARIES_MS[i]
            } else {
                BUCKET_BOUNDARIES_MS[BUCKET_BOUNDARIES_MS.len() - 1] + 1
            };
        }
    }
    0
}

fn global_agg() -> &'static Mutex<LiveLatencyAggregator> {
    static AGG: OnceLock<Mutex<LiveLatencyAggregator>> = OnceLock::new();
    AGG.get_or_init(|| Mutex::new(LiveLatencyAggregator::new()))
}

pub(crate) fn record_latency(stage: LatencyStage, ms: u64) {
    if !enabled() {
        return;
    }
    let label = match stage {
        LatencyStage::CaptureToRequest => "capture_to_request",
        LatencyStage::Ttfb => "ttfb",
        LatencyStage::FirstAudio => "first_audio",
        LatencyStage::PlaybackEnd => "playback_end",
    };
    emit("live_latency", label, &format!("ms={ms}"));
    if let Ok(mut agg) = global_agg().lock() {
        agg.record(stage, ms);
    }
}

pub(crate) fn increment_underruns() {
    if !enabled() {
        return;
    }
    if let Ok(mut agg) = global_agg().lock() {
        agg.increment_underruns();
        let total = agg.underruns;
        emit("live_latency", "underrun", &format!("total={total}"));
    }
}

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
