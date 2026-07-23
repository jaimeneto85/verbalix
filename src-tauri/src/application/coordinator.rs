use crate::{
    application::{OverlayPort, SelectionPort},
    domain::{
        AiProvider, SelectionEvent, SelectionSnapshot, SelectionState, TransformOperation,
        TransformRequest, TransformResult, VerbalixError,
    },
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct SelectionCoordinator {
    selection: Arc<dyn SelectionPort>,
    overlay: Arc<dyn OverlayPort>,
    provider: Arc<dyn AiProvider>,
    state: Mutex<SelectionState>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{OverlayPort, SelectionPort},
        domain::{Rect, TextRange, TransformResult},
    };
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct FakeSelection {
        current: Mutex<SelectionSnapshot>,
        replacements: Mutex<Vec<String>>,
    }

    impl SelectionPort for FakeSelection {
        fn permission_granted(&self, _prompt: bool) -> bool {
            true
        }

        fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
            self.current
                .lock()
                .map(|current| current.clone())
                .map_err(|_| VerbalixError::LocalFailure)
        }

        fn replace(
            &self,
            expected: &SelectionSnapshot,
            text: &str,
        ) -> Result<(), VerbalixError> {
            let current = self
                .current
                .lock()
                .map_err(|_| VerbalixError::LocalFailure)?;
            if !current.same_target(expected) {
                return Err(VerbalixError::StaleSelection);
            }
            self.replacements
                .lock()
                .map_err(|_| VerbalixError::LocalFailure)?
                .push(text.to_owned());
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeOverlay {
        toolbar_count: Mutex<usize>,
        notes: Mutex<Vec<String>>,
        hidden_count: Mutex<usize>,
    }

    impl OverlayPort for FakeOverlay {
        fn show_toolbar(&self, _bounds: Rect) -> Result<(), VerbalixError> {
            *self
                .toolbar_count
                .lock()
                .map_err(|_| VerbalixError::LocalFailure)? += 1;
            Ok(())
        }

        fn show_note(&self, _bounds: Rect, text: &str) -> Result<(), VerbalixError> {
            self.notes
                .lock()
                .map_err(|_| VerbalixError::LocalFailure)?
                .push(text.to_owned());
            Ok(())
        }

        fn hide_all(&self) -> Result<(), VerbalixError> {
            *self
                .hidden_count
                .lock()
                .map_err(|_| VerbalixError::LocalFailure)? += 1;
            Ok(())
        }
    }

    struct FakeProvider {
        result: String,
    }

    #[async_trait]
    impl AiProvider for FakeProvider {
        async fn transform(
            &self,
            request: &TransformRequest,
            _access_token: &str,
        ) -> Result<TransformResult, VerbalixError> {
            Ok(TransformResult {
                request_id: request.request_id,
                source_language: "pt".to_owned(),
                target_language: Some("en".to_owned()),
                result: self.result.clone(),
            })
        }
    }

    struct ErrorProvider;

    #[async_trait]
    impl AiProvider for ErrorProvider {
        async fn transform(
            &self,
            _request: &TransformRequest,
            _access_token: &str,
        ) -> Result<TransformResult, VerbalixError> {
            Err(VerbalixError::ProviderTimeout)
        }
    }

    struct WrongRequestProvider;

    #[async_trait]
    impl AiProvider for WrongRequestProvider {
        async fn transform(
            &self,
            _request: &TransformRequest,
            _access_token: &str,
        ) -> Result<TransformResult, VerbalixError> {
            Ok(TransformResult {
                request_id: Uuid::new_v4(),
                source_language: "pt".to_owned(),
                target_language: Some("en".to_owned()),
                result: "result".to_owned(),
            })
        }
    }

    fn snapshot(writable: bool) -> SelectionSnapshot {
        SelectionSnapshot::new(
            42,
            "com.example.editor".to_owned(),
            "Olá 👋🏽 APIClient".to_owned(),
            TextRange {
                location: 5,
                length: 16,
            },
            Rect {
                x: 100.0,
                y: 200.0,
                width: 90.0,
                height: 18.0,
            },
            writable,
        )
    }

    fn coordinator(
        selection: Arc<FakeSelection>,
        overlay: Arc<FakeOverlay>,
    ) -> SelectionCoordinator {
        SelectionCoordinator::new(
            selection,
            overlay,
            Arc::new(FakeProvider {
                result: "Hello 👋🏽 APIClient".to_owned(),
            }),
        )
    }

    #[test]
    fn only_latest_candidate_opens_toolbar() {
        let first = snapshot(true);
        let mut second = snapshot(true);
        second.text = "Outra seleção".to_owned();
        let selection = Arc::new(FakeSelection {
            current: Mutex::new(second.clone()),
            replacements: Mutex::new(Vec::new()),
        });
        let overlay = Arc::new(FakeOverlay::default());
        let coordinator = coordinator(selection, overlay.clone());

        coordinator
            .dispatch(SelectionEvent::Candidate(first.clone()))
            .unwrap();
        coordinator
            .dispatch(SelectionEvent::Candidate(second.clone()))
            .unwrap();
        coordinator
            .dispatch(SelectionEvent::DebounceElapsed(first.id))
            .unwrap();
        assert_eq!(*overlay.toolbar_count.lock().unwrap(), 0);
        coordinator
            .dispatch(SelectionEvent::DebounceElapsed(second.id))
            .unwrap();
        assert_eq!(*overlay.toolbar_count.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn writable_selection_is_replaced_after_revalidation() {
        let current = snapshot(true);
        let selection = Arc::new(FakeSelection {
            current: Mutex::new(current.clone()),
            replacements: Mutex::new(Vec::new()),
        });
        let overlay = Arc::new(FakeOverlay::default());
        let coordinator = coordinator(selection.clone(), overlay);
        coordinator
            .dispatch(SelectionEvent::Candidate(current.clone()))
            .unwrap();
        coordinator
            .dispatch(SelectionEvent::DebounceElapsed(current.id))
            .unwrap();
        let request = coordinator.request_for(
            TransformOperation::Translate,
            current.text.clone(),
            None,
        );

        coordinator.transform(request, "token").await.unwrap();

        assert_eq!(
            selection.replacements.lock().unwrap().as_slice(),
            ["Hello 👋🏽 APIClient"]
        );
    }

    #[tokio::test]
    async fn read_only_selection_uses_note_without_replacement() {
        let current = snapshot(false);
        let selection = Arc::new(FakeSelection {
            current: Mutex::new(current.clone()),
            replacements: Mutex::new(Vec::new()),
        });
        let overlay = Arc::new(FakeOverlay::default());
        let coordinator = coordinator(selection.clone(), overlay.clone());
        coordinator
            .dispatch(SelectionEvent::Candidate(current.clone()))
            .unwrap();
        coordinator
            .dispatch(SelectionEvent::DebounceElapsed(current.id))
            .unwrap();
        let request = coordinator.request_for(
            TransformOperation::Translate,
            current.text.clone(),
            None,
        );

        coordinator.transform(request, "token").await.unwrap();

        assert!(selection.replacements.lock().unwrap().is_empty());
        assert_eq!(
            overlay.notes.lock().unwrap().as_slice(),
            ["Hello 👋🏽 APIClient"]
        );
    }

    #[tokio::test]
    async fn changed_selection_blocks_remote_result() {
        let current = snapshot(true);
        let selection = Arc::new(FakeSelection {
            current: Mutex::new(current.clone()),
            replacements: Mutex::new(Vec::new()),
        });
        let overlay = Arc::new(FakeOverlay::default());
        let coordinator = coordinator(selection.clone(), overlay);
        coordinator
            .dispatch(SelectionEvent::Candidate(current.clone()))
            .unwrap();
        coordinator
            .dispatch(SelectionEvent::DebounceElapsed(current.id))
            .unwrap();
        let request = coordinator.request_for(
            TransformOperation::Translate,
            current.text.clone(),
            None,
        );
        selection.current.lock().unwrap().text = "Seleção mudou".to_owned();

        let result = coordinator.transform(request, "token").await;

        assert!(matches!(result, Err(VerbalixError::StaleSelection)));
        assert!(selection.replacements.lock().unwrap().is_empty());
    }

    #[test]
    fn invalidation_hides_overlays_and_clears_snapshot() {
        let current = snapshot(true);
        let selection = Arc::new(FakeSelection {
            current: Mutex::new(current.clone()),
            replacements: Mutex::new(Vec::new()),
        });
        let overlay = Arc::new(FakeOverlay::default());
        let coordinator = coordinator(selection, overlay.clone());
        coordinator
            .dispatch(SelectionEvent::Candidate(current))
            .unwrap();

        coordinator.dispatch(SelectionEvent::Invalidated).unwrap();

        assert!(coordinator.current_snapshot().is_none());
        assert_eq!(*overlay.hidden_count.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn provider_timeout_never_writes_or_shows_result() {
        let current = snapshot(true);
        let selection = Arc::new(FakeSelection {
            current: Mutex::new(current.clone()),
            replacements: Mutex::new(Vec::new()),
        });
        let overlay = Arc::new(FakeOverlay::default());
        let coordinator = SelectionCoordinator::new(
            selection.clone(),
            overlay.clone(),
            Arc::new(ErrorProvider),
        );
        coordinator
            .dispatch(SelectionEvent::Candidate(current.clone()))
            .unwrap();
        coordinator
            .dispatch(SelectionEvent::DebounceElapsed(current.id))
            .unwrap();
        let request = coordinator.request_for(
            TransformOperation::Translate,
            current.text.clone(),
            None,
        );

        let result = coordinator.transform(request, "token").await;

        assert!(matches!(result, Err(VerbalixError::ProviderTimeout)));
        assert!(selection.replacements.lock().unwrap().is_empty());
        assert!(overlay.notes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn mismatched_request_id_never_writes() {
        let current = snapshot(true);
        let selection = Arc::new(FakeSelection {
            current: Mutex::new(current.clone()),
            replacements: Mutex::new(Vec::new()),
        });
        let coordinator = SelectionCoordinator::new(
            selection.clone(),
            Arc::new(FakeOverlay::default()),
            Arc::new(WrongRequestProvider),
        );
        coordinator
            .dispatch(SelectionEvent::Candidate(current.clone()))
            .unwrap();
        coordinator
            .dispatch(SelectionEvent::DebounceElapsed(current.id))
            .unwrap();
        let request = coordinator.request_for(
            TransformOperation::Translate,
            current.text.clone(),
            None,
        );

        let result = coordinator.transform(request, "token").await;

        assert!(matches!(result, Err(VerbalixError::InvalidResponse)));
        assert!(selection.replacements.lock().unwrap().is_empty());
    }

    #[test]
    fn undo_rejects_when_transformed_content_is_no_longer_intact() {
        let current = snapshot(true);
        let selection = Arc::new(FakeSelection {
            current: Mutex::new(current.clone()),
            replacements: Mutex::new(Vec::new()),
        });
        let coordinator = coordinator(selection.clone(), Arc::new(FakeOverlay::default()));
        coordinator
            .dispatch(SelectionEvent::Candidate(current))
            .unwrap();
        selection.current.lock().unwrap().text = "edited again".to_owned();

        let result = coordinator.undo("transformed");

        assert!(matches!(result, Err(VerbalixError::StaleSelection)));
        assert!(selection.replacements.lock().unwrap().is_empty());
    }

    #[test]
    fn oversized_capture_is_invalidated_before_provider_use() {
        let mut current = snapshot(true);
        current.text = "a".repeat(12_001);
        let selection = Arc::new(FakeSelection {
            current: Mutex::new(current),
            replacements: Mutex::new(Vec::new()),
        });
        let overlay = Arc::new(FakeOverlay::default());
        let coordinator = coordinator(selection, overlay.clone());

        let result = coordinator.refresh_selection();

        assert!(matches!(result, Err(VerbalixError::TextTooLong)));
        assert_eq!(*overlay.hidden_count.lock().unwrap(), 1);
    }
}

impl SelectionCoordinator {
    pub fn new(
        selection: Arc<dyn SelectionPort>,
        overlay: Arc<dyn OverlayPort>,
        provider: Arc<dyn AiProvider>,
    ) -> Self {
        Self {
            selection,
            overlay,
            provider,
            state: Mutex::new(SelectionState::Idle),
        }
    }

    pub fn current_snapshot(&self) -> Option<SelectionSnapshot> {
        let state = self.state.lock().ok()?;
        match &*state {
            SelectionState::Candidate(snapshot)
            | SelectionState::ToolbarVisible(snapshot)
            | SelectionState::ResultVisible(snapshot)
            | SelectionState::Processing { snapshot, .. } => Some(snapshot.clone()),
            SelectionState::Idle => None,
        }
    }

    pub fn dispatch(&self, event: SelectionEvent) -> Result<(), VerbalixError> {
        let mut state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        match event {
            SelectionEvent::Candidate(snapshot) => {
                *state = SelectionState::Candidate(snapshot);
            }
            SelectionEvent::DebounceElapsed(id) => {
                if let SelectionState::Candidate(snapshot) = &*state {
                    if snapshot.id == id {
                        self.overlay.show_toolbar(snapshot.bounds)?;
                        *state = SelectionState::ToolbarVisible(snapshot.clone());
                    }
                }
            }
            SelectionEvent::ActionStarted(request_id) => {
                if let SelectionState::ToolbarVisible(snapshot) = &*state {
                    *state = SelectionState::Processing {
                        snapshot: snapshot.clone(),
                        request_id,
                    };
                }
            }
            SelectionEvent::ResultReady(request_id) => {
                if let SelectionState::Processing {
                    snapshot,
                    request_id: active_request,
                } = &*state
                {
                    if *active_request == request_id {
                        *state = SelectionState::ResultVisible(snapshot.clone());
                    }
                }
            }
            SelectionEvent::Invalidated => {
                self.overlay.hide_all()?;
                *state = SelectionState::Idle;
            }
        }
        Ok(())
    }

    pub fn refresh_selection(&self) -> Result<Option<SelectionSnapshot>, VerbalixError> {
        let snapshot = self.selection.capture()?;
        if snapshot.text.chars().count() > 12_000 {
            self.dispatch(SelectionEvent::Invalidated)?;
            return Err(VerbalixError::TextTooLong);
        }
        let unchanged = self
            .current_snapshot()
            .is_some_and(|current| current.same_target(&snapshot));
        if !unchanged {
            self.dispatch(SelectionEvent::Candidate(snapshot.clone()))?;
        }
        Ok(Some(snapshot))
    }

    pub async fn transform(
        &self,
        request: TransformRequest,
        access_token: &str,
    ) -> Result<TransformResult, VerbalixError> {
        request.validate()?;
        let snapshot = self
            .current_snapshot()
            .filter(|snapshot| snapshot.text == request.text)
            .ok_or(VerbalixError::StaleSelection)?;
        self.dispatch(SelectionEvent::ActionStarted(request.request_id))?;
        let response = self.provider.transform(&request, access_token).await?;
        if response.request_id != request.request_id || response.result.trim().is_empty() {
            return Err(VerbalixError::InvalidResponse);
        }
        let active = self
            .current_snapshot()
            .filter(|current| current.same_target(&snapshot))
            .ok_or(VerbalixError::StaleSelection)?;
        if active.writable {
            self.selection.replace(&active, &response.result)?;
        } else {
            self.overlay.show_note(active.bounds, &response.result)?;
        }
        self.dispatch(SelectionEvent::ResultReady(request.request_id))?;
        Ok(response)
    }

    pub fn undo(&self, transformed_text: &str) -> Result<(), VerbalixError> {
        let snapshot = self
            .current_snapshot()
            .ok_or(VerbalixError::StaleSelection)?;
        let current = self.selection.capture()?;
        if current.text != transformed_text {
            return Err(VerbalixError::StaleSelection);
        }
        self.selection.replace(&current, &snapshot.text)
    }

    pub fn request_for(
        &self,
        operation: TransformOperation,
        text: String,
        preferences: Option<crate::domain::TransformPreferences>,
    ) -> TransformRequest {
        TransformRequest {
            request_id: Uuid::new_v4(),
            operation,
            text,
            preferences,
        }
    }
}
