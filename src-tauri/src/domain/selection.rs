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
