use super::*;
use crate::domain::{
    Rect, SelectionElementIdentity, TextRange, TransformOperation, TransformRequest,
    TransformResult,
};
use async_trait::async_trait;
use std::sync::Mutex;
use tokio::sync::Notify;
use uuid::Uuid;

struct MutableSelection {
    current: Mutex<SelectionSnapshot>,
    writes: Mutex<Vec<(Uuid, String)>>,
}

impl SelectionPort for MutableSelection {
    fn permission_granted(&self, _prompt: bool) -> bool {
        true
    }

    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        Ok(self.current.lock().unwrap().clone())
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
struct EventOverlay {
    events: Mutex<Vec<&'static str>>,
    fail_hide: bool,
}

impl OverlayPort for EventOverlay {
    fn show_toolbar(&self, _bounds: Rect) -> Result<(), VerbalixError> {
        self.events.lock().unwrap().push("toolbar");
        Ok(())
    }

    fn show_note(&self, _bounds: Rect, _text: &str) -> Result<(), VerbalixError> {
        self.events.lock().unwrap().push("note");
        Ok(())
    }

    fn show_preview(
        &self,
        _bounds: Rect,
        _request_id: Uuid,
        _text: &str,
    ) -> Result<(), VerbalixError> {
        self.events.lock().unwrap().push("preview");
        Ok(())
    }

    fn show_undo(&self, _bounds: Rect, _text: &str) -> Result<(), VerbalixError> {
        self.events.lock().unwrap().push("undo");
        Ok(())
    }

    fn hide_all(&self) -> Result<(), VerbalixError> {
        self.events.lock().unwrap().push("hide");
        if self.fail_hide {
            Err(VerbalixError::LocalFailure)
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
struct ImmediateProvider {
    calls: Mutex<usize>,
}

#[async_trait]
impl AiProvider for ImmediateProvider {
    async fn transform(
        &self,
        request: &TransformRequest,
        _access_token: &str,
    ) -> Result<TransformResult, VerbalixError> {
        *self.calls.lock().unwrap() += 1;
        Ok(result(request.request_id))
    }
}

struct BlockingProvider {
    calls: Mutex<usize>,
    started: Notify,
    release: Notify,
}

#[async_trait]
impl AiProvider for BlockingProvider {
    async fn transform(
        &self,
        request: &TransformRequest,
        _access_token: &str,
    ) -> Result<TransformResult, VerbalixError> {
        *self.calls.lock().unwrap() += 1;
        self.started.notify_one();
        self.release.notified().await;
        Ok(result(request.request_id))
    }
}

fn result(request_id: Uuid) -> TransformResult {
    TransformResult {
        request_id,
        source_language: "pt".to_owned(),
        target_language: Some("en".to_owned()),
        result: "translated".to_owned(),
    }
}

fn snapshot(pid: i32, identifier: &str) -> SelectionSnapshot {
    SelectionSnapshot::new(
        pid,
        format!("pid:{pid}"),
        "same text".to_owned(),
        TextRange {
            location: 3,
            length: 9,
        },
        Rect {
            x: 10.0,
            y: 20.0,
            width: 70.0,
            height: 18.0,
        },
        true,
    )
    .with_element_identity(SelectionElementIdentity {
        role: "AXTextArea".to_owned(),
        subrole: None,
        identifier: Some(identifier.to_owned()),
        frame: Rect {
            x: 1.0,
            y: 2.0,
            width: 300.0,
            height: 120.0,
        },
    })
}

fn request() -> TransformRequest {
    TransformRequest {
        request_id: Uuid::new_v4(),
        operation: TransformOperation::Translate,
        text: "same text".to_owned(),
        preferences: None,
    }
}

fn ready(
    provider: Arc<dyn AiProvider>,
    fail_hide: bool,
) -> (
    Arc<SelectionCoordinator>,
    Arc<MutableSelection>,
    Arc<EventOverlay>,
    SelectionSnapshot,
) {
    let captured = snapshot(42, "editor-a");
    let selection = Arc::new(MutableSelection {
        current: Mutex::new(captured.clone()),
        writes: Mutex::new(Vec::new()),
    });
    let overlay = Arc::new(EventOverlay {
        events: Mutex::new(Vec::new()),
        fail_hide,
    });
    let coordinator = Arc::new(SelectionCoordinator::new(
        selection.clone(),
        overlay.clone(),
        provider,
    ));
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(captured.clone())))
        .unwrap();
    coordinator
        .dispatch(SelectionEvent::DebounceElapsed(captured.id))
        .unwrap();
    (coordinator, selection, overlay, captured)
}

#[tokio::test]
async fn equivalent_candidate_preserves_the_original_lease_and_writes_once() {
    let provider = Arc::new(ImmediateProvider::default());
    let (coordinator, selection, overlay, captured) = ready(provider.clone(), false);
    let input = request();
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();
    let mut equivalent = captured.clone();
    equivalent.id = Uuid::new_v4();
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(equivalent)))
        .unwrap();

    assert!(matches!(
        &*coordinator.state.lock().unwrap(),
        SelectionState::Processing {
            snapshot,
            request_id
        } if snapshot.id == captured.id && *request_id == input.request_id
    ));
    coordinator
        .transform(captured.id, input, "token", false)
        .await
        .unwrap();

    assert_eq!(*provider.calls.lock().unwrap(), 1);
    assert_eq!(selection.writes.lock().unwrap().len(), 1);
    assert!(!overlay.events.lock().unwrap().contains(&"hide"));
}

#[tokio::test]
async fn same_text_in_another_pid_supersedes_before_the_provider() {
    assert_superseded_before_provider(snapshot(84, "editor-a")).await;
}

#[tokio::test]
async fn same_text_and_pid_with_another_ax_identifier_supersedes() {
    assert_superseded_before_provider(snapshot(42, "editor-b")).await;
}

async fn assert_superseded_before_provider(next: SelectionSnapshot) {
    let provider = Arc::new(ImmediateProvider::default());
    let (coordinator, selection, _overlay, captured) = ready(provider.clone(), false);
    let input = request();
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();
    let next_id = next.id;
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(next)))
        .unwrap();

    assert!(matches!(
        coordinator
            .transform(captured.id, input, "token", false)
            .await,
        Err(VerbalixError::StaleSelection)
    ));
    assert_eq!(*provider.calls.lock().unwrap(), 0);
    assert!(selection.writes.lock().unwrap().is_empty());
    assert_eq!(coordinator.current_snapshot().unwrap().id, next_id);
}

#[path = "coordinator_supersede_race_tests.rs"]
mod races;
