use super::*;
use crate::domain::{Rect, TextRange, TransformRequest, TransformResult};
use async_trait::async_trait;
use std::sync::{Barrier, Mutex};
use uuid::Uuid;

struct RecapturingSelection;

impl SelectionPort for RecapturingSelection {
    fn permission_granted(&self, _prompt: bool) -> bool {
        true
    }

    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        Ok(SelectionSnapshot::new(
            42,
            "com.editor".to_owned(),
            "same target".to_owned(),
            TextRange {
                location: 2,
                length: 11,
            },
            Rect {
                x: 100.0,
                y: 200.0,
                width: 80.0,
                height: 20.0,
            },
            true,
        ))
    }

    fn replace(&self, _expected: &SelectionSnapshot, _text: &str) -> Result<(), VerbalixError> {
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

struct DeniedSelection;

impl SelectionPort for DeniedSelection {
    fn permission_granted(&self, _prompt: bool) -> bool {
        false
    }

    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        Err(VerbalixError::PermissionDenied)
    }

    fn replace(&self, _expected: &SelectionSnapshot, _text: &str) -> Result<(), VerbalixError> {
        Err(VerbalixError::PermissionDenied)
    }

    fn restore(
        &self,
        _expected: &SelectionSnapshot,
        _transformed_text: &str,
    ) -> Result<(), VerbalixError> {
        Err(VerbalixError::PermissionDenied)
    }
}

#[derive(Default)]
struct RecordingOverlay {
    toolbar_count: Mutex<usize>,
}

impl OverlayPort for RecordingOverlay {
    fn show_toolbar(&self, _bounds: Rect) -> Result<(), VerbalixError> {
        *self.toolbar_count.lock().unwrap() += 1;
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

struct UnusedProvider;

#[async_trait]
impl AiProvider for UnusedProvider {
    async fn transform(
        &self,
        _request: &TransformRequest,
        _access_token: &str,
    ) -> Result<TransformResult, VerbalixError> {
        Err(VerbalixError::ProviderRejected)
    }
}

#[test]
fn equivalent_recapture_returns_the_active_candidate_id_for_debounce() {
    let overlay = Arc::new(RecordingOverlay::default());
    let coordinator = SelectionCoordinator::new(
        Arc::new(RecapturingSelection),
        overlay.clone(),
        Arc::new(UnusedProvider),
    );
    let original = coordinator.refresh_selection().unwrap().unwrap();
    let recaptured = coordinator.refresh_selection().unwrap().unwrap();

    assert_eq!(recaptured.id, original.id);
    coordinator
        .dispatch(SelectionEvent::DebounceElapsed(recaptured.id))
        .unwrap();
    assert_eq!(*overlay.toolbar_count.lock().unwrap(), 1);
}

#[test]
fn concurrent_polling_and_observer_debounces_show_one_toolbar() {
    let overlay = Arc::new(RecordingOverlay::default());
    let coordinator = Arc::new(SelectionCoordinator::new(
        Arc::new(RecapturingSelection),
        overlay.clone(),
        Arc::new(UnusedProvider),
    ));
    let start = Arc::new(Barrier::new(3));
    let callers = (0..2)
        .map(|_| {
            let coordinator = coordinator.clone();
            let start = start.clone();
            std::thread::spawn(move || {
                start.wait();
                coordinator.refresh_selection().unwrap().unwrap().id
            })
        })
        .collect::<Vec<_>>();
    start.wait();
    let ids = callers
        .into_iter()
        .map(|caller| caller.join().unwrap())
        .collect::<Vec<_>>();

    for id in ids {
        coordinator
            .dispatch(SelectionEvent::DebounceElapsed(id))
            .unwrap();
    }

    assert_eq!(*overlay.toolbar_count.lock().unwrap(), 1);
}

#[test]
fn permission_denied_stops_before_any_toolbar_intention() {
    let overlay = Arc::new(RecordingOverlay::default());
    let coordinator = SelectionCoordinator::new(
        Arc::new(DeniedSelection),
        overlay.clone(),
        Arc::new(UnusedProvider),
    );

    assert!(matches!(
        coordinator.refresh_selection(),
        Err(VerbalixError::PermissionDenied)
    ));
    assert!(coordinator.current_snapshot().is_none());
    assert_eq!(*overlay.toolbar_count.lock().unwrap(), 0);
}
