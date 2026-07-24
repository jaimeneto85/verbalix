use super::*;
use crate::domain::{Rect, TextRange, TransformOperation, TransformRequest, TransformResult};
use async_trait::async_trait;
use std::sync::Mutex;
use uuid::Uuid;

struct RecordingSelection {
    snapshot: Mutex<SelectionSnapshot>,
    writes: Mutex<Vec<(Uuid, String)>>,
}

impl SelectionPort for RecordingSelection {
    fn permission_granted(&self, _prompt: bool) -> bool {
        true
    }

    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        Ok(self.snapshot.lock().unwrap().clone())
    }

    fn replace(&self, expected: &SelectionSnapshot, text: &str) -> Result<(), VerbalixError> {
        self.writes
            .lock()
            .unwrap()
            .push((expected.id, text.to_owned()));
        Ok(())
    }

    fn restore(
        &self,
        _expected: &SelectionSnapshot,
        _transformed_text: &str,
    ) -> Result<(), VerbalixError> {
        Ok(())
    }
}

#[derive(Default)]
struct RecordingOverlay {
    events: Mutex<Vec<String>>,
    fail_undo: bool,
}

impl OverlayPort for RecordingOverlay {
    fn show_toolbar(&self, _bounds: Rect) -> Result<(), VerbalixError> {
        self.events.lock().unwrap().push("toolbar".to_owned());
        Ok(())
    }

    fn show_note(&self, _bounds: Rect, text: &str) -> Result<(), VerbalixError> {
        self.events.lock().unwrap().push(format!("note:{text}"));
        Ok(())
    }

    fn show_preview(
        &self,
        _bounds: Rect,
        _request_id: Uuid,
        text: &str,
    ) -> Result<(), VerbalixError> {
        self.events.lock().unwrap().push(format!("preview:{text}"));
        Ok(())
    }

    fn show_undo(&self, _bounds: Rect, text: &str) -> Result<(), VerbalixError> {
        if self.fail_undo {
            return Err(VerbalixError::LocalFailure);
        }
        self.events.lock().unwrap().push(format!("undo:{text}"));
        Ok(())
    }

    fn hide_all(&self) -> Result<(), VerbalixError> {
        self.events.lock().unwrap().push("hide".to_owned());
        Ok(())
    }
}

#[derive(Default)]
struct RecordingProvider {
    calls: Mutex<Vec<(Uuid, TransformOperation)>>,
}

#[async_trait]
impl AiProvider for RecordingProvider {
    async fn transform(
        &self,
        request: &TransformRequest,
        _access_token: &str,
    ) -> Result<TransformResult, VerbalixError> {
        self.calls
            .lock()
            .unwrap()
            .push((request.request_id, request.operation));
        let result = match request.operation {
            TransformOperation::Translate => "translated",
            TransformOperation::Improve => "improved",
        };
        Ok(TransformResult {
            request_id: request.request_id,
            source_language: "pt".to_owned(),
            target_language: Some("en".to_owned()),
            result: result.to_owned(),
        })
    }
}

fn snapshot(pid: i32, text: &str, writable: bool) -> SelectionSnapshot {
    SelectionSnapshot::new(
        pid,
        format!("pid:{pid}"),
        text.to_owned(),
        TextRange {
            location: 4,
            length: text.encode_utf16().count() as i64,
        },
        Rect {
            x: 100.0,
            y: 200.0,
            width: 80.0,
            height: 20.0,
        },
        writable,
    )
}

fn request(operation: TransformOperation, text: &str) -> TransformRequest {
    TransformRequest {
        request_id: Uuid::new_v4(),
        operation,
        text: text.to_owned(),
        preferences: None,
    }
}

fn ready(
    provider: Arc<dyn AiProvider>,
    fail_undo: bool,
) -> (
    SelectionCoordinator,
    Arc<RecordingSelection>,
    Arc<RecordingOverlay>,
    SelectionSnapshot,
) {
    let captured = snapshot(42, "Olá 👩🏽‍💻", true);
    let selection = Arc::new(RecordingSelection {
        snapshot: Mutex::new(captured.clone()),
        writes: Mutex::new(Vec::new()),
    });
    let overlay = Arc::new(RecordingOverlay {
        events: Mutex::new(Vec::new()),
        fail_undo,
    });
    let coordinator = SelectionCoordinator::new(selection.clone(), overlay.clone(), provider);
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(captured.clone())))
        .unwrap();
    coordinator
        .dispatch(SelectionEvent::DebounceElapsed(captured.id))
        .unwrap();
    (coordinator, selection, overlay, captured)
}

#[tokio::test]
async fn translate_and_improve_keep_independent_provider_and_write_contracts() {
    for (operation, expected) in [
        (TransformOperation::Translate, "translated"),
        (TransformOperation::Improve, "improved"),
    ] {
        let provider = Arc::new(RecordingProvider::default());
        let (coordinator, selection, _overlay, captured) = ready(provider.clone(), false);
        let input = request(operation, &captured.text);
        coordinator
            .begin_transform(captured.id, input.request_id)
            .unwrap();
        coordinator
            .transform(captured.id, input.clone(), "token", false)
            .await
            .unwrap();

        assert_eq!(
            provider.calls.lock().unwrap().as_slice(),
            [(input.request_id, operation)]
        );
        assert_eq!(
            selection.writes.lock().unwrap().as_slice(),
            [(captured.id, expected.to_owned())]
        );
    }
}

#[tokio::test]
async fn different_target_with_same_text_cancels_before_provider_or_write() {
    let provider = Arc::new(RecordingProvider::default());
    let (coordinator, selection, overlay, captured) = ready(provider.clone(), false);
    let input = request(TransformOperation::Translate, &captured.text);
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();

    *selection.snapshot.lock().unwrap() = snapshot(84, &captured.text, true);
    let polled = coordinator.refresh_selection().unwrap().unwrap();
    assert_ne!(polled.id, captured.id);
    assert!(matches!(
        coordinator
            .transform(captured.id, input, "token", false)
            .await,
        Err(VerbalixError::StaleSelection)
    ));

    assert!(provider.calls.lock().unwrap().is_empty());
    assert!(selection.writes.lock().unwrap().is_empty());
    assert!(overlay.events.lock().unwrap().contains(&"hide".to_owned()));
    assert_eq!(coordinator.current_snapshot().unwrap().id, polled.id);
}

#[tokio::test]
async fn transient_invalidation_preserves_the_pinned_target() {
    let provider = Arc::new(RecordingProvider::default());
    let (coordinator, selection, _overlay, captured) = ready(provider.clone(), false);
    let input = request(TransformOperation::Translate, &captured.text);
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();
    coordinator
        .dispatch(SelectionEvent::TransientInvalidated)
        .unwrap();
    coordinator
        .transform(captured.id, input, "token", false)
        .await
        .unwrap();

    assert_eq!(provider.calls.lock().unwrap().len(), 1);
    assert_eq!(selection.writes.lock().unwrap()[0].0, captured.id);
}

#[tokio::test]
async fn real_invalidation_terminates_the_pinned_action_without_provider_or_write() {
    let provider = Arc::new(RecordingProvider::default());
    let (coordinator, selection, _overlay, captured) = ready(provider.clone(), false);
    let input = request(TransformOperation::Improve, &captured.text);
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();
    coordinator.dispatch(SelectionEvent::Invalidated).unwrap();

    assert!(matches!(
        coordinator
            .transform(captured.id, input, "token", false)
            .await,
        Err(VerbalixError::StaleSelection)
    ));
    assert!(provider.calls.lock().unwrap().is_empty());
    assert!(selection.writes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn successful_direct_write_remains_applied_when_undo_feedback_fails() {
    let provider = Arc::new(RecordingProvider::default());
    let (coordinator, selection, _overlay, captured) = ready(provider, true);
    let input = request(TransformOperation::Improve, &captured.text);
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();
    coordinator
        .transform(captured.id, input, "token", false)
        .await
        .unwrap();

    assert_eq!(selection.writes.lock().unwrap().len(), 1);
    assert!(matches!(
        coordinator.state.lock().unwrap().clone(),
        SelectionState::Applied { .. }
    ));
}

#[tokio::test]
async fn successful_preview_apply_remains_applied_when_undo_feedback_fails() {
    let provider = Arc::new(RecordingProvider::default());
    let (coordinator, selection, _overlay, captured) = ready(provider, true);
    let input = request(TransformOperation::Improve, &captured.text);
    let request_id = input.request_id;
    coordinator
        .begin_transform(captured.id, request_id)
        .unwrap();
    coordinator
        .transform(captured.id, input, "token", true)
        .await
        .unwrap();
    assert!(selection.writes.lock().unwrap().is_empty());

    assert_eq!(coordinator.apply_preview(request_id).unwrap(), "improved");
    assert_eq!(selection.writes.lock().unwrap().len(), 1);
    assert!(matches!(
        coordinator.state.lock().unwrap().clone(),
        SelectionState::Applied { .. }
    ));
}
