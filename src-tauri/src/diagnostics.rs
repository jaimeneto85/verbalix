use crate::domain::{SelectionSnapshot, VerbalixError};
#[cfg(target_os = "macos")]
use crate::platform::macos_focus::{AxCategory, AxStage, ExtractionOrigin};
use std::sync::OnceLock;
#[cfg(target_os = "macos")]
use std::{collections::HashMap, sync::Mutex};

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
    emit(
        "accessibility",
        "status",
        if trusted {
            "trusted=true"
        } else {
            "trusted=false"
        },
    );
}

pub fn ai_readiness(status: &str) {
    emit("ai", "readiness", &format!("status={status}"));
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
        "snapshot_id={} pid={} range_location={} range_length={} bounds={:.1},{:.1},{:.1},{:.1} geometry_source={} writable={}",
        snapshot.id,
        snapshot.pid,
        snapshot.range.location,
        snapshot.range.length,
        snapshot.bounds.x,
        snapshot.bounds.y,
        snapshot.bounds.width,
        snapshot.bounds.height,
        snapshot
            .geometry_source
            .map(|source| source.as_str())
            .unwrap_or("unknown"),
        snapshot.writable
    )
}

fn lifecycle_metadata(origin: &'static str) -> String {
    format!("origin={origin}")
}

fn error_code(error: &VerbalixError) -> &'static str {
    match error {
        VerbalixError::PermissionDenied => "permission_denied",
        VerbalixError::SelectionUnavailable => "selection_unavailable",
        VerbalixError::ProtectedField => "protected_field",
        VerbalixError::StaleSelection => "stale_selection",
        VerbalixError::TextTooLong => "text_too_long",
        VerbalixError::Unauthenticated => "unauthenticated",
        VerbalixError::ProviderNotConfigured => "provider_not_configured",
        VerbalixError::ProviderTimeout => "provider_timeout",
        VerbalixError::ProviderRejected => "provider_rejected",
        VerbalixError::InvalidResponse => "invalid_response",
        #[cfg(not(target_os = "macos"))]
        VerbalixError::UnsupportedPlatform => "unsupported_platform",
        VerbalixError::LocalFailure => "local_failure",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Rect, TextRange};
    #[cfg(target_os = "macos")]
    use crate::platform::macos_focus::{AxCategory, AxStage, ExtractionOrigin};

    #[test]
    fn snapshot_diagnostics_never_include_selected_text() {
        let snapshot = SelectionSnapshot::new(
            42,
            "pid:42".to_owned(),
            "secret selected text".to_owned(),
            TextRange {
                location: 3,
                length: 8,
            },
            Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            },
            true,
        );

        let metadata = snapshot_metadata(&snapshot);

        assert!(!metadata.contains("secret selected text"));
        assert!(metadata.contains("pid=42"));
        assert!(metadata.contains("range_location=3"));
        assert!(metadata.contains("geometry_source=unknown"));
    }

    #[test]
    fn geometry_source_uses_a_sanitized_stable_label() {
        let snapshot = SelectionSnapshot::new(
            42,
            "pid:42".to_owned(),
            "secret selected text".to_owned(),
            TextRange {
                location: 3,
                length: 8,
            },
            Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            },
            true,
        )
        .with_geometry_source(crate::domain::GeometrySource::FocusedElement);

        let metadata = snapshot_metadata(&snapshot);

        assert!(metadata.contains("geometry_source=focused_element"));
        assert!(!metadata.contains("secret selected text"));
    }

    #[test]
    fn marker_geometry_and_read_only_state_are_reported_without_content() {
        let snapshot = SelectionSnapshot::new(
            42,
            "pid:42".to_owned(),
            "private marker content".to_owned(),
            TextRange {
                location: 3,
                length: 8,
            },
            Rect {
                x: -1200.0,
                y: 20.0,
                width: 80.0,
                height: 18.0,
            },
            false,
        )
        .with_geometry_source(crate::domain::GeometrySource::TextMarkerRange);

        let metadata = snapshot_metadata(&snapshot);

        assert!(metadata.contains("geometry_source=text_marker_range"));
        assert!(metadata.contains("writable=false"));
        assert!(!metadata.contains("private marker content"));
        assert!(!metadata.contains("pid:42"));
    }

    #[test]
    fn permission_failure_uses_a_sanitized_stable_code() {
        assert_eq!(
            error_code(&VerbalixError::PermissionDenied),
            "permission_denied"
        );
    }

    #[test]
    fn lifecycle_metadata_contains_only_a_bounded_origin() {
        let metadata = lifecycle_metadata("dock_reopen");

        assert_eq!(metadata, "origin=dock_reopen");
        assert!(!metadata.contains("secret selected text"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn ax_diagnostics_emit_only_when_a_stage_category_transitions() {
        let mut last = HashMap::new();
        let key = (AxStage::SelectedText, ExtractionOrigin::SelectedText);

        assert!(should_emit_ax_transition(
            &mut last,
            key,
            AxCategory::NoValue
        ));
        assert!(!should_emit_ax_transition(
            &mut last,
            key,
            AxCategory::NoValue
        ));
        assert!(should_emit_ax_transition(
            &mut last,
            key,
            AxCategory::Success
        ));
        assert!(should_emit_ax_transition(
            &mut last,
            key,
            AxCategory::NoValue
        ));
        assert!(should_emit_ax_transition(
            &mut last,
            (AxStage::SelectedText, ExtractionOrigin::TextMarker),
            AxCategory::NoValue
        ));
        assert!(!should_emit_ax_transition(
            &mut last,
            (AxStage::SelectedText, ExtractionOrigin::TextMarker),
            AxCategory::NoValue
        ));
    }
}
