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
#[serde(rename_all = "camelCase")]
pub struct TextRange {
    pub location: i64,
    pub length: i64,
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
            writable,
            captured_at_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or_default(),
        }
    }

    pub fn same_target(&self, other: &Self) -> bool {
        self.pid == other.pid
            && self.bundle_id == other.bundle_id
            && self.text == other.text
            && self.range == other.range
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
    },
    ResultVisible(SelectionSnapshot),
}

#[derive(Clone, Debug)]
pub enum SelectionEvent {
    Candidate(SelectionSnapshot),
    DebounceElapsed(Uuid),
    ActionStarted(Uuid),
    ResultReady(Uuid),
    Invalidated,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OverlayKind {
    Toolbar,
    Note,
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
}
