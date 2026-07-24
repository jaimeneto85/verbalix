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
        if let SelectionEvent::Candidate(snapshot) = event {
            return self.store_candidate(*snapshot);
        }
        if matches!(event, SelectionEvent::TransientInvalidated) {
            return self.invalidate(true);
        }
        if matches!(event, SelectionEvent::Invalidated) {
            return self.invalidate(false);
        }
        let mut state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        match event {
            SelectionEvent::Candidate(_) => unreachable!(),
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
            SelectionEvent::TransientInvalidated | SelectionEvent::Invalidated => unreachable!(),
        }
        Ok(())
    }

    fn invalidate(&self, transient: bool) -> Result<(), VerbalixError> {
        {
            let mut state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
            if transient && matches!(&*state, SelectionState::Processing { .. }) {
                diagnostics::coordinator("transient_invalidation_ignored", None);
                return Ok(());
            }
            diagnostics::coordinator(
                if transient {
                    "transient_invalidation_applied"
                } else {
                    "invalidated"
                },
                None,
            );
            *state = SelectionState::Idle;
        }
        self.overlay.hide_all()
    }

    fn store_candidate(&self, snapshot: SelectionSnapshot) -> Result<(), VerbalixError> {
        let superseded = {
            let mut state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
            if let SelectionState::Processing {
                snapshot: active, ..
            } = &*state
            {
                if active.same_target(&snapshot) {
                    diagnostics::coordinator("candidate_preserved_processing", Some(active));
                    return Ok(());
                }
                diagnostics::coordinator("candidate_superseded_processing", Some(&snapshot));
                *state = SelectionState::Candidate(snapshot.clone());
                true
            } else {
                diagnostics::coordinator("candidate_stored", Some(&snapshot));
                *state = SelectionState::Candidate(snapshot.clone());
                false
            }
        };
        if superseded && self.overlay.hide_all().is_err() {
            diagnostics::coordinator("candidate_supersede_hide_failed", Some(&snapshot));
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

#[cfg(test)]
#[path = "coordinator_transform_regression_tests.rs"]
mod transform_regression_tests;

#[cfg(test)]
#[path = "coordinator_supersede_tests.rs"]
mod supersede_tests;
