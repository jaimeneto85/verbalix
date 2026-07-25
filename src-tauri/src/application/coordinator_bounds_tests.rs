use super::*;
use crate::domain::{
    Rect, SelectionEvent, SelectionSnapshot, TextRange, TransformRequest, TransformResult,
    VerbalixError,
};
use async_trait::async_trait;
use std::sync::Mutex;
use uuid::Uuid;

struct FixedSelection {
    snapshot: Mutex<SelectionSnapshot>,
}

impl SelectionPort for FixedSelection {
    fn permission_granted(&self, _prompt: bool) -> bool {
        true
    }

    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        Ok(self.snapshot.lock().unwrap().clone())
    }

    fn replace(&self, _expected: &SelectionSnapshot, text: &str) -> Result<(), VerbalixError> {
        self.snapshot.lock().unwrap().text = text.to_owned();
        Ok(())
    }

    fn restore(&self, _expected: &SelectionSnapshot, original: &str) -> Result<(), VerbalixError> {
        self.snapshot.lock().unwrap().text = original.to_owned();
        Ok(())
    }
}

#[derive(Default)]
struct SilentOverlay;

impl OverlayPort for SilentOverlay {
    fn show_toolbar(&self, _bounds: Rect) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn show_note(&self, _bounds: Rect, _text: &str) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn show_preview(
        &self,
        _bounds: Rect,
        _request_id: Uuid,
        _text: &str,
    ) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn show_undo(&self, _bounds: Rect, _text: &str) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn hide_all(&self) -> Result<(), VerbalixError> {
        Ok(())
    }
}

struct NoopProvider;

#[async_trait]
impl crate::domain::AiProvider for NoopProvider {
    async fn transform(
        &self,
        request: &TransformRequest,
        _token: &str,
    ) -> Result<TransformResult, VerbalixError> {
        Ok(TransformResult {
            request_id: request.request_id,
            source_language: "pt".to_owned(),
            target_language: Some("en".to_owned()),
            result: "translated".to_owned(),
        })
    }
}

fn bounds_snapshot() -> SelectionSnapshot {
    SelectionSnapshot::new(
        1,
        "com.test".to_owned(),
        "hello".to_owned(),
        TextRange {
            location: 0,
            length: 5,
        },
        Rect {
            x: 10.0,
            y: 20.0,
            width: 50.0,
            height: 15.0,
        },
        true,
    )
}

fn make_coordinator() -> (SelectionCoordinator, SelectionSnapshot) {
    let snap = bounds_snapshot();
    let selection = std::sync::Arc::new(FixedSelection {
        snapshot: Mutex::new(snap.clone()),
    });
    let overlay = std::sync::Arc::new(SilentOverlay);
    let provider = std::sync::Arc::new(NoopProvider);
    let coordinator = SelectionCoordinator::new(selection, overlay, provider);
    (coordinator, snap)
}

#[test]
fn last_known_bounds_is_some_after_candidate_dispatched() {
    let (coordinator, snap) = make_coordinator();

    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(snap.clone())))
        .unwrap();

    assert_eq!(coordinator.last_known_bounds(), Some(snap.bounds));
}

#[test]
fn last_known_bounds_is_none_after_invalidated() {
    let (coordinator, snap) = make_coordinator();

    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(snap.clone())))
        .unwrap();
    assert!(coordinator.last_known_bounds().is_some());

    coordinator.dispatch(SelectionEvent::Invalidated).unwrap();

    assert!(coordinator.last_known_bounds().is_none());
}

#[test]
fn transient_invalidation_clears_bounds_when_state_is_candidate() {
    let (coordinator, snap) = make_coordinator();

    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(snap.clone())))
        .unwrap();
    assert!(coordinator.last_known_bounds().is_some());

    coordinator
        .dispatch(SelectionEvent::TransientInvalidated)
        .unwrap();

    assert!(coordinator.last_known_bounds().is_none());
}

#[test]
fn transient_invalidation_preserves_bounds_when_state_is_processing() {
    let (coordinator, snap) = make_coordinator();

    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(snap.clone())))
        .unwrap();
    coordinator
        .dispatch(SelectionEvent::DebounceElapsed(snap.id))
        .unwrap();

    let request_id = Uuid::new_v4();
    coordinator.begin_transform(snap.id, request_id).unwrap();

    coordinator
        .dispatch(SelectionEvent::TransientInvalidated)
        .unwrap();

    assert_eq!(coordinator.last_known_bounds(), Some(snap.bounds));
}
