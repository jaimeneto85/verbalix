use crate::{
    application::{OverlayPort, SelectionPort},
    diagnostics,
    domain::{
        AiProvider, SelectionEvent, SelectionSnapshot, SelectionState, TransformRequest,
        TransformResult, VerbalixError,
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
            | SelectionState::PreviewVisible { snapshot, .. }
            | SelectionState::Applied { snapshot, .. }
            | SelectionState::Processing { snapshot, .. } => Some(snapshot.clone()),
            SelectionState::Idle => None,
        }
    }

    pub fn dispatch(&self, event: SelectionEvent) -> Result<(), VerbalixError> {
        let mut state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        match event {
            SelectionEvent::Candidate(snapshot) => {
                let snapshot = *snapshot;
                diagnostics::coordinator("candidate_stored", Some(&snapshot));
                *state = SelectionState::Candidate(snapshot);
            }
            SelectionEvent::DebounceElapsed(id) => {
                if let SelectionState::Candidate(snapshot) = &*state {
                    if snapshot.id == id {
                        diagnostics::coordinator("debounce_accepted", Some(snapshot));
                        self.overlay.show_toolbar(snapshot.bounds)?;
                        *state = SelectionState::ToolbarVisible(snapshot.clone());
                    } else {
                        diagnostics::coordinator("debounce_ignored_id", Some(snapshot));
                    }
                } else {
                    diagnostics::coordinator("debounce_ignored_state", None);
                }
            }
            SelectionEvent::ActionStarted(request_id) => {
                if let SelectionState::ToolbarVisible(snapshot)
                | SelectionState::Processing { snapshot, .. }
                | SelectionState::ResultVisible(snapshot)
                | SelectionState::PreviewVisible { snapshot, .. } = &*state
                {
                    *state = SelectionState::Processing {
                        snapshot: snapshot.clone(),
                        request_id,
                    };
                }
            }
            SelectionEvent::ResultReady(request_id) => {
                if let SelectionState::Processing {
                    snapshot,
                    request_id: active,
                } = &*state
                {
                    if *active == request_id {
                        *state = SelectionState::ResultVisible(snapshot.clone());
                    }
                }
            }
            SelectionEvent::Invalidated => {
                diagnostics::coordinator("invalidated", None);
                self.overlay.hide_all()?;
                *state = SelectionState::Idle;
            }
        }
        Ok(())
    }

    pub fn refresh_selection(&self) -> Result<Option<SelectionSnapshot>, VerbalixError> {
        let snapshot = self.selection.capture()?;
        diagnostics::capture_success(&snapshot);
        if snapshot.text.chars().count() > 12_000 {
            self.dispatch(SelectionEvent::Invalidated)?;
            return Err(VerbalixError::TextTooLong);
        }
        if let Some(active) = self
            .current_snapshot()
            .filter(|current| current.same_target(&snapshot))
        {
            diagnostics::coordinator("equivalent_target_reused", Some(&active));
            return Ok(Some(active));
        }
        diagnostics::coordinator("new_target", Some(&snapshot));
        self.dispatch(SelectionEvent::Candidate(Box::new(snapshot.clone())))?;
        Ok(Some(snapshot))
    }

    pub async fn transform(
        &self,
        request: TransformRequest,
        access_token: &str,
        preview_writable: bool,
    ) -> Result<TransformResult, VerbalixError> {
        request.validate()?;
        let snapshot = self
            .current_snapshot()
            .filter(|snapshot| snapshot.text == request.text)
            .ok_or(VerbalixError::StaleSelection)?;
        self.dispatch(SelectionEvent::ActionStarted(request.request_id))?;
        let response = match self.provider.transform(&request, access_token).await {
            Ok(response) => response,
            Err(error) => return self.fail(request.request_id, error),
        };
        match self.finish_transform(&request, &snapshot, &response, preview_writable) {
            Ok(()) => Ok(response),
            Err(error) => self.fail(request.request_id, error),
        }
    }

    fn finish_transform(
        &self,
        request: &TransformRequest,
        snapshot: &SelectionSnapshot,
        response: &TransformResult,
        preview_writable: bool,
    ) -> Result<(), VerbalixError> {
        if !self.is_active_request(request.request_id)? {
            return Err(VerbalixError::StaleSelection);
        }
        if response.request_id != request.request_id || response.result.trim().is_empty() {
            return Err(VerbalixError::InvalidResponse);
        }
        let active = self
            .current_snapshot()
            .filter(|current| current.same_target(snapshot))
            .ok_or(VerbalixError::StaleSelection)?;
        if !active.writable {
            self.overlay.show_note(active.bounds, &response.result)?;
            return self.dispatch(SelectionEvent::ResultReady(request.request_id));
        }
        if preview_writable {
            self.overlay
                .show_preview(active.bounds, request.request_id, &response.result)?;
            return self.set_state(SelectionState::PreviewVisible {
                snapshot: active,
                request_id: request.request_id,
                result: response.result.clone(),
            });
        }
        self.selection.replace(&active, &response.result)?;
        self.overlay.show_undo(active.bounds, &response.result)?;
        self.set_state(SelectionState::Applied {
            snapshot: active,
            transformed_text: response.result.clone(),
        })
    }

    pub fn apply_preview(&self, request_id: Uuid) -> Result<String, VerbalixError> {
        let (snapshot, result) = {
            let state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
            match &*state {
                SelectionState::PreviewVisible {
                    snapshot,
                    request_id: active,
                    result,
                } if *active == request_id => (snapshot.clone(), result.clone()),
                _ => return Err(VerbalixError::StaleSelection),
            }
        };
        self.selection.replace(&snapshot, &result)?;
        self.overlay.show_undo(snapshot.bounds, &result)?;
        self.set_state(SelectionState::Applied {
            snapshot,
            transformed_text: result.clone(),
        })?;
        Ok(result)
    }

    pub fn undo(&self, transformed_text: &str) -> Result<(), VerbalixError> {
        let snapshot = {
            let state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
            match &*state {
                SelectionState::Applied {
                    snapshot,
                    transformed_text: active,
                } if active == transformed_text => snapshot.clone(),
                _ => return Err(VerbalixError::StaleSelection),
            }
        };
        self.selection.restore(&snapshot, transformed_text)?;
        self.dispatch(SelectionEvent::Invalidated)
    }

    fn fail<T>(&self, request_id: Uuid, error: VerbalixError) -> Result<T, VerbalixError> {
        self.recover_request(request_id)?;
        Err(error)
    }

    fn is_active_request(&self, request_id: Uuid) -> Result<bool, VerbalixError> {
        let state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        Ok(matches!(
            &*state,
            SelectionState::Processing {
                request_id: active,
                ..
            } if *active == request_id
        ))
    }

    fn recover_request(&self, request_id: Uuid) -> Result<(), VerbalixError> {
        let mut state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        if let SelectionState::Processing {
            snapshot,
            request_id: active,
        } = &*state
        {
            if *active == request_id {
                *state = SelectionState::ToolbarVisible(snapshot.clone());
            }
        }
        Ok(())
    }

    fn set_state(&self, next: SelectionState) -> Result<(), VerbalixError> {
        *self.state.lock().map_err(|_| VerbalixError::LocalFailure)? = next;
        Ok(())
    }
}

#[cfg(test)]
#[path = "coordinator_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "coordinator_identity_tests.rs"]
mod identity_tests;
