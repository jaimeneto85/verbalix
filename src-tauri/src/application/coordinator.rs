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
