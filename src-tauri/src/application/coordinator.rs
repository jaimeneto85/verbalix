use crate::{
    application::{OverlayPort, SelectionPort},
    diagnostics,
    domain::{AiProvider, SelectionEvent, SelectionSnapshot, SelectionState, VerbalixError},
};
use std::sync::{Arc, Mutex};

pub struct SelectionCoordinator {
    pub(super) selection: Arc<dyn SelectionPort>,
    pub(super) overlay: Arc<dyn OverlayPort>,
    pub(super) provider: Arc<dyn AiProvider>,
    pub(super) state: Mutex<SelectionState>,
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
                if matches!(&*state, SelectionState::Processing { .. }) {
                    diagnostics::coordinator("candidate_ignored_processing", Some(&snapshot));
                    return Ok(());
                }
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
            SelectionEvent::TransientInvalidated => {
                if matches!(&*state, SelectionState::Processing { .. }) {
                    diagnostics::coordinator("transient_invalidation_ignored", None);
                    return Ok(());
                }
                diagnostics::coordinator("transient_invalidation_applied", None);
                self.overlay.hide_all()?;
                *state = SelectionState::Idle;
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
}

#[cfg(test)]
#[path = "coordinator_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "coordinator_identity_tests.rs"]
mod identity_tests;
