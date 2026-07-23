use super::*;
use crate::domain::{Rect, TextRange};
use async_trait::async_trait;
use std::sync::Mutex;

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
