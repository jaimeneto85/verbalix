use super::*;
use crate::domain::{Rect, TextRange, TransformOperation, TransformRequest, TransformResult};
use async_trait::async_trait;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
use tokio::sync::Notify;
use uuid::Uuid;

struct FakeSelection {
    snapshot: Mutex<SelectionSnapshot>,
    replacements: Mutex<Vec<String>>,
    fail_write: bool,
}

impl SelectionPort for FakeSelection {
    fn permission_granted(&self, _prompt: bool) -> bool {
        true
    }

    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        Ok(self.snapshot.lock().unwrap().clone())
    }

    fn replace(&self, _expected: &SelectionSnapshot, text: &str) -> Result<(), VerbalixError> {
        if self.fail_write {
            return Err(VerbalixError::StaleSelection);
        }
        self.replacements.lock().unwrap().push(text.to_owned());
        self.snapshot.lock().unwrap().text = text.to_owned();
        Ok(())
    }

    fn restore(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
    ) -> Result<(), VerbalixError> {
        if self.snapshot.lock().unwrap().text != transformed_text {
            return Err(VerbalixError::StaleSelection);
        }
        self.replacements
            .lock()
            .unwrap()
            .push(expected.text.clone());
        Ok(())
    }
}

#[derive(Default)]
struct FakeOverlay {
    events: Mutex<Vec<String>>,
    fail_result: bool,
}

impl OverlayPort for FakeOverlay {
    fn show_toolbar(&self, _bounds: Rect) -> Result<(), VerbalixError> {
        self.events.lock().unwrap().push("toolbar".to_owned());
        Ok(())
    }

    fn show_note(&self, _bounds: Rect, text: &str) -> Result<(), VerbalixError> {
        if self.fail_result {
            return Err(VerbalixError::LocalFailure);
        }
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
        self.events.lock().unwrap().push(format!("undo:{text}"));
        Ok(())
    }

    fn hide_all(&self) -> Result<(), VerbalixError> {
        self.events.lock().unwrap().push("hide".to_owned());
        Ok(())
    }
}

struct FakeProvider;

#[async_trait]
impl AiProvider for FakeProvider {
    async fn transform(
        &self,
        request: &TransformRequest,
        _token: &str,
    ) -> Result<TransformResult, VerbalixError> {
        Ok(TransformResult {
            request_id: request.request_id,
            source_language: "pt".to_owned(),
            target_language: Some("en".to_owned()),
            result: "result".to_owned(),
        })
    }
}

struct OutOfOrderProvider {
    calls: AtomicUsize,
    started: Notify,
    release: Notify,
}

#[async_trait]
impl AiProvider for OutOfOrderProvider {
    async fn transform(
        &self,
        request: &TransformRequest,
        _token: &str,
    ) -> Result<TransformResult, VerbalixError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            self.started.notify_one();
            self.release.notified().await;
        }
        Ok(TransformResult {
            request_id: request.request_id,
            source_language: "pt".to_owned(),
            target_language: Some("en".to_owned()),
            result: format!("result-{call}"),
        })
    }
}

fn snapshot(writable: bool) -> SelectionSnapshot {
    SelectionSnapshot::new(
        42,
        "com.editor".to_owned(),
        "Olá 👩🏽‍💻".to_owned(),
        TextRange {
            location: 2,
            length: 9,
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

fn request(text: &str) -> TransformRequest {
    TransformRequest {
        request_id: Uuid::new_v4(),
        operation: TransformOperation::Translate,
        text: text.to_owned(),
        preferences: None,
    }
}

async fn execute(
    coordinator: &SelectionCoordinator,
    input: TransformRequest,
    preview: bool,
) -> Result<TransformResult, VerbalixError> {
    let snapshot_id = coordinator.current_snapshot().unwrap().id;
    coordinator.begin_transform(snapshot_id, input.request_id)?;
    coordinator
        .transform(snapshot_id, input, "token", preview)
        .await
}

fn ready(
    writable: bool,
    fail_write: bool,
    fail_result: bool,
) -> (SelectionCoordinator, Arc<FakeSelection>, Arc<FakeOverlay>) {
    let captured = snapshot(writable);
    let selection = Arc::new(FakeSelection {
        snapshot: Mutex::new(captured.clone()),
        replacements: Mutex::new(Vec::new()),
        fail_write,
    });
    let overlay = Arc::new(FakeOverlay {
        events: Mutex::new(Vec::new()),
        fail_result,
    });
    let coordinator =
        SelectionCoordinator::new(selection.clone(), overlay.clone(), Arc::new(FakeProvider));
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(captured.clone())))
        .unwrap();
    coordinator
        .dispatch(SelectionEvent::DebounceElapsed(captured.id))
        .unwrap();
    (coordinator, selection, overlay)
}

#[tokio::test]
async fn preview_waits_for_explicit_apply() {
    let (coordinator, selection, overlay) = ready(true, false, false);
    let input = request("Olá 👩🏽‍💻");
    let id = input.request_id;
    execute(&coordinator, input, true).await.unwrap();
    assert!(selection.replacements.lock().unwrap().is_empty());
    coordinator.apply_preview(id).unwrap();
    assert_eq!(
        selection.replacements.lock().unwrap().as_slice(),
        ["result"]
    );
    assert!(overlay
        .events
        .lock()
        .unwrap()
        .contains(&"preview:result".to_owned()));
}

#[tokio::test]
async fn desktop_flow_reaches_toolbar_preview_apply_and_undo() {
    let (coordinator, selection, overlay) = ready(true, false, false);
    let input = request("Olá 👩🏽‍💻");
    let id = input.request_id;

    execute(&coordinator, input, true).await.unwrap();
    coordinator.apply_preview(id).unwrap();
    coordinator.undo("result").unwrap();

    assert_eq!(
        overlay.events.lock().unwrap().as_slice(),
        ["toolbar", "preview:result", "undo:result", "hide"]
    );
    assert_eq!(
        selection.replacements.lock().unwrap().as_slice(),
        ["result", "Olá 👩🏽‍💻"]
    );
}

#[tokio::test]
async fn write_failure_recovers_to_toolbar_state() {
    let (coordinator, _selection, _overlay) = ready(true, true, false);
    let first = request("Olá 👩🏽‍💻");
    assert!(execute(&coordinator, first, false).await.is_err());
    let second = request("Olá 👩🏽‍💻");
    assert!(execute(&coordinator, second, true).await.is_ok());
}

#[tokio::test]
async fn overlay_failure_recovers_to_toolbar_state() {
    let (coordinator, _selection, _overlay) = ready(false, false, true);
    let first = request("Olá 👩🏽‍💻");
    assert!(execute(&coordinator, first, false).await.is_err());
    let second = request("Olá 👩🏽‍💻");
    assert!(execute(&coordinator, second, false).await.is_err());
}

#[tokio::test]
async fn read_only_marker_result_uses_note_without_attempting_mutation() {
    let (coordinator, selection, overlay) = ready(false, false, false);
    let input = request("Olá 👩🏽‍💻");

    execute(&coordinator, input, false).await.unwrap();

    assert!(selection.replacements.lock().unwrap().is_empty());
    assert_eq!(
        overlay.events.lock().unwrap().as_slice(),
        ["toolbar", "note:result"]
    );
}

#[path = "coordinator_request_tests.rs"]
mod request_tests;
