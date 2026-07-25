use super::*;

fn snapshot(text: &str, range: TextRange) -> SelectionSnapshot {
    SelectionSnapshot::new(
        8,
        "com.example".to_owned(),
        text.to_owned(),
        range,
        Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        true,
    )
}

#[test]
fn utf16_range_counts_surrogate_pairs_without_changing_utf8_text() {
    let text = "A👩🏽‍💻ção";
    let utf16_length = text.encode_utf16().count() as i64;
    let captured = snapshot(
        text,
        TextRange {
            location: 3,
            length: utf16_length,
        },
    );
    assert_eq!(captured.text, text);
    assert_eq!(captured.range.length, 11);
    assert_eq!(captured.text.chars().count(), 8);
}

#[test]
fn target_identity_includes_utf16_range_and_pid() {
    let first = snapshot(
        "mesmo texto",
        TextRange {
            location: 0,
            length: 10,
        },
    );
    let mut moved = first.clone();
    moved.range.location = 12;
    assert!(!first.same_target(&moved));
    moved.range = first.range;
    moved.pid = 99;
    assert!(!first.same_target(&moved));
}

#[test]
fn target_identity_includes_bundle_and_text_but_not_visual_metadata() {
    let first = snapshot(
        "same text",
        TextRange {
            location: 4,
            length: 9,
        },
    );
    let mut changed = first.clone();
    changed.bundle_id = "com.other.editor".to_owned();
    assert!(!first.same_target(&changed));
    changed.bundle_id = first.bundle_id.clone();
    changed.text = "different".to_owned();
    assert!(!first.same_target(&changed));
    changed.text = first.text.clone();
    changed.id = Uuid::new_v4();
    changed.bounds.x = -1440.0;
    changed.geometry_source = Some(GeometrySource::TextMarkerRange);
    changed.writable = false;
    assert!(first.same_target(&changed));
    changed.extraction_strategy = SelectionExtractionStrategy::ValueRange;
    assert!(!first.same_target(&changed));
    changed.extraction_strategy = first.extraction_strategy;
    changed.element_identity = Some(SelectionElementIdentity {
        role: "AXTextArea".to_owned(),
        subrole: None,
        frame: Rect {
            x: 2.0,
            y: 3.0,
            width: 100.0,
            height: 40.0,
        },
    });
    assert!(!first.same_target(&changed));
}

#[test]
fn every_extraction_strategy_is_a_distinct_target_identity() {
    let strategies = [
        SelectionExtractionStrategy::SelectedText,
        SelectionExtractionStrategy::StringForRange,
        SelectionExtractionStrategy::ValueRange,
        SelectionExtractionStrategy::TextMarker,
    ];
    let base = snapshot(
        "same",
        TextRange {
            location: 0,
            length: 4,
        },
    );
    for expected in strategies {
        for current in strategies {
            let first = base.clone().with_extraction_strategy(expected);
            let second = base.clone().with_extraction_strategy(current);
            assert_eq!(first.same_target(&second), expected == current);
        }
    }
}

#[test]
fn native_identifier_is_private_from_serde_and_debug_but_keeps_target_identity() {
    let sentinel = "private-ax-identifier";
    let first = snapshot(
        "same",
        TextRange {
            location: 0,
            length: 4,
        },
    )
    .with_native_element_identifier(Some(sentinel.to_owned()));
    let second = first
        .clone()
        .with_native_element_identifier(Some("another-native-target".to_owned()));
    let serialized = serde_json::to_string(&Some(first.clone())).unwrap();
    let debug = format!("{:?}", SelectionState::Candidate(first.clone()));
    assert!(!serialized.contains(sentinel));
    assert!(!serialized.contains("nativeElementIdentifier"));
    assert!(!serialized.contains("identifier"));
    assert!(!debug.contains(sentinel));
    assert!(debug.contains("<redacted>"));
    assert!(!first.same_target(&second));
}
