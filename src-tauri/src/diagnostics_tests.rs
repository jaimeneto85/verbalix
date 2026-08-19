use super::*;
use crate::domain::{Rect, TextRange};
#[cfg(target_os = "macos")]
use crate::platform::macos_focus::{AxCategory, AxStage, ExtractionOrigin};

#[test]
fn p50_and_p95_of_known_latency_distribution() {
    let mut agg = LiveLatencyAggregator::new();
    for _ in 0..50 {
        agg.record(LatencyStage::CaptureToRequest, 80);
    }
    for _ in 0..50 {
        agg.record(LatencyStage::CaptureToRequest, 200);
    }
    assert_eq!(
        agg.p50(LatencyStage::CaptureToRequest),
        100,
        "50th percentile of [80×50, 200×50] should fall in ≤100ms bucket"
    );
    assert_eq!(
        agg.p95(LatencyStage::CaptureToRequest),
        200,
        "95th percentile of [80×50, 200×50] should fall in ≤200ms bucket"
    );
}

#[test]
fn p95_at_overflow_bucket() {
    let mut agg = LiveLatencyAggregator::new();
    for _ in 0..90 {
        agg.record(LatencyStage::Ttfb, 100);
    }
    for _ in 0..10 {
        agg.record(LatencyStage::Ttfb, 9_999);
    }
    assert_eq!(
        agg.p50(LatencyStage::Ttfb),
        100,
        "p50 falls in ≤100ms bucket"
    );
    let p95 = agg.p95(LatencyStage::Ttfb);
    assert!(p95 > 5000, "p95 is in overflow bucket (>5000ms), got {p95}");
}

#[test]
fn all_stages_aggregate_independently() {
    let mut agg = LiveLatencyAggregator::new();
    for _ in 0..10 {
        agg.record(LatencyStage::CaptureToRequest, 80);
    }
    for _ in 0..10 {
        agg.record(LatencyStage::FirstAudio, 500);
    }
    assert_eq!(agg.p50(LatencyStage::CaptureToRequest), 100);
    assert_eq!(agg.p50(LatencyStage::FirstAudio), 500);
    assert_eq!(agg.p50(LatencyStage::Ttfb), 0, "empty stage returns 0");
}

#[test]
fn underrun_counter_increments() {
    let mut agg = LiveLatencyAggregator::new();
    assert_eq!(agg.underruns, 0);
    agg.increment_underruns();
    agg.increment_underruns();
    assert_eq!(agg.underruns, 2);
}

#[test]
fn latency_ms_at_exact_bucket_boundary_falls_in_that_bucket() {
    let mut agg = LiveLatencyAggregator::new();
    agg.record(LatencyStage::FirstAudio, 50);
    assert_eq!(
        agg.p50(LatencyStage::FirstAudio),
        50,
        "ms=50 (exactly the first boundary) must land in the ≤50ms bucket"
    );

    let mut agg2 = LiveLatencyAggregator::new();
    agg2.record(LatencyStage::FirstAudio, 5001);
    let p50 = agg2.p50(LatencyStage::FirstAudio);
    assert!(
        p50 > 5000,
        "ms=5001 exceeds all named boundaries; expected overflow bucket, got {p50}"
    );
}

#[test]
fn single_sample_p50_and_p95_agree() {
    let mut agg = LiveLatencyAggregator::new();
    agg.record(LatencyStage::PlaybackEnd, 300);
    assert_eq!(agg.p50(LatencyStage::PlaybackEnd), 300);
    assert_eq!(agg.p95(LatencyStage::PlaybackEnd), 300);
}

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
    assert!(!metadata.contains("pid=42"));
    assert!(!metadata.contains("range_location"));
    assert!(!metadata.contains("range_length"));
    assert!(!metadata.contains("bounds="));
    assert!(metadata.contains("geometry_source=unknown"));
    assert!(metadata.contains("extraction_strategy=selected_text"));
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

#[test]
fn virtual_mic_metadata_reports_only_status_and_counters() {
    let metadata = virtual_mic_metadata(VirtualMicStatus::Installed, 12, 3);

    assert_eq!(metadata, "status=installed buffer_depth=12 underruns=3");
    assert!(!metadata.contains("com.verbalix.virtualmic"));
    assert!(!metadata.contains("Verbalix Microphone"));
}

#[test]
fn virtual_mic_metadata_covers_all_status_variants() {
    assert_eq!(
        virtual_mic_metadata(VirtualMicStatus::NotInstalled, 0, 0),
        "status=not_installed buffer_depth=0 underruns=0"
    );
    assert_eq!(
        virtual_mic_metadata(VirtualMicStatus::IncompatibleVersion, 0, 0),
        "status=incompatible_version buffer_depth=0 underruns=0"
    );
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
