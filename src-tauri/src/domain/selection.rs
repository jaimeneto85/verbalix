use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometrySource {
    SelectedRange,
    TextMarkerRange,
    FocusedElement,
    Cursor,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionExtractionStrategy {
    #[default]
    SelectedText,
    StringForRange,
    ValueRange,
    TextMarker,
}

impl SelectionExtractionStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SelectedText => "selected_text",
            Self::StringForRange => "string_for_range",
            Self::ValueRange => "value_range",
            Self::TextMarker => "text_marker",
        }
    }
}

impl GeometrySource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SelectedRange => "selected_range",
            Self::TextMarkerRange => "text_marker_range",
            Self::FocusedElement => "focused_element",
            Self::Cursor => "cursor",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRange {
    pub location: i64,
    pub length: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionElementIdentity {
    pub role: String,
    pub subrole: Option<String>,
    pub frame: Rect,
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct NativeElementIdentifier(String);

impl NativeElementIdentifier {
    pub(crate) fn new(identifier: String) -> Option<Self> {
        (!identifier.trim().is_empty()).then_some(Self(identifier))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for NativeElementIdentifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("NativeElementIdentifier(<redacted>)")
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionSnapshot {
    pub id: Uuid,
    pub pid: i32,
    pub bundle_id: String,
    pub text: String,
    pub range: TextRange,
    pub bounds: Rect,
    pub geometry_source: Option<GeometrySource>,
    #[serde(default)]
    pub extraction_strategy: SelectionExtractionStrategy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element_identity: Option<SelectionElementIdentity>,
    #[serde(skip)]
    pub(crate) native_element_identifier: Option<NativeElementIdentifier>,
    pub writable: bool,
    pub captured_at_ms: u128,
}

impl SelectionSnapshot {
    pub fn new(
        pid: i32,
        bundle_id: String,
        text: String,
        range: TextRange,
        bounds: Rect,
        writable: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            pid,
            bundle_id,
            text,
            range,
            bounds,
            geometry_source: None,
            extraction_strategy: SelectionExtractionStrategy::SelectedText,
            element_identity: None,
            native_element_identifier: None,
            writable,
            captured_at_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or_default(),
        }
    }

    pub fn with_geometry_source(mut self, source: GeometrySource) -> Self {
        self.geometry_source = Some(source);
        self
    }

    pub fn with_extraction_strategy(mut self, strategy: SelectionExtractionStrategy) -> Self {
        self.extraction_strategy = strategy;
        self
    }

    pub fn with_element_identity(mut self, identity: SelectionElementIdentity) -> Self {
        self.element_identity = Some(identity);
        self
    }

    pub(crate) fn with_native_element_identifier(mut self, identifier: Option<String>) -> Self {
        self.native_element_identifier = identifier.and_then(NativeElementIdentifier::new);
        self
    }

    pub(crate) fn native_element_identifier(&self) -> Option<&str> {
        self.native_element_identifier
            .as_ref()
            .map(NativeElementIdentifier::as_str)
    }

    pub fn same_target(&self, other: &Self) -> bool {
        self.pid == other.pid
            && self.bundle_id == other.bundle_id
            && self.text == other.text
            && self.range == other.range
            && self.element_identity == other.element_identity
            && self.native_element_identifier == other.native_element_identifier
            && self.extraction_strategy == other.extraction_strategy
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SelectionState {
    Idle,
    Candidate(SelectionSnapshot),
    ToolbarVisible(SelectionSnapshot),
    Processing {
        snapshot: SelectionSnapshot,
        request_id: Uuid,
    },
    PreviewVisible {
        snapshot: SelectionSnapshot,
        request_id: Uuid,
        result: String,
    },
    Applied {
        snapshot: SelectionSnapshot,
        transformed_text: String,
        mutation_id: Uuid,
    },
    ResultVisible(SelectionSnapshot),
}

#[derive(Clone, Debug)]
pub enum SelectionEvent {
    Candidate(Box<SelectionSnapshot>),
    DebounceElapsed(Uuid),
    TransientInvalidated,
    Invalidated,
}

#[cfg(test)]
mod tests {
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
}
