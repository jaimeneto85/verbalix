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
#[path = "selection_tests.rs"]
mod tests;
