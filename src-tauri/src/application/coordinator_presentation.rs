use super::SelectionCoordinator;
use crate::{
    application::coordinator::SelectionPresentation,
    application::{PublicationGuard, TransformLease},
    diagnostics,
    domain::{SelectionSnapshot, SelectionState, VerbalixError},
};
use std::sync::Arc;
use uuid::Uuid;

impl SelectionCoordinator {
    pub(super) fn install_presentation(
        &self,
        snapshot: SelectionSnapshot,
    ) -> Result<(), VerbalixError> {
        let mut presentation = self
            .presentation
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)?;
        if let Some(previous) = presentation.take() {
            previous.guard.cancel();
        }
        *presentation = Some(SelectionPresentation {
            guard: Arc::new(TransformLease::new(snapshot.id, Uuid::nil())),
            snapshot,
        });
        Ok(())
    }

    pub(super) fn cancel_presentation(&self) -> Result<(), VerbalixError> {
        let mut presentation = self
            .presentation
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)?;
        if let Some(presentation) = presentation.take() {
            presentation.guard.cancel();
        }
        Ok(())
    }

    pub(super) fn show_candidate_toolbar(&self, id: Uuid) -> Result<(), VerbalixError> {
        let Some((snapshot, guard)) = self.candidate_presentation(id)? else {
            return Ok(());
        };
        self.overlay
            .show_toolbar_guarded(snapshot.bounds, guard.clone())?;
        let mut state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        let candidate_matches =
            matches!(&*state, SelectionState::Candidate(current) if current.id == id);
        let presentation = self
            .presentation
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)?;
        let presentation_matches = presentation.as_ref().is_some_and(|presentation| {
            presentation.snapshot.id == id && Arc::ptr_eq(&presentation.guard, &guard)
        });
        if candidate_matches && presentation_matches && guard.may_publish() {
            diagnostics::coordinator("debounce_accepted", Some(&snapshot));
            *state = SelectionState::ToolbarVisible(snapshot);
        }
        Ok(())
    }

    fn candidate_presentation(
        &self,
        id: Uuid,
    ) -> Result<Option<(SelectionSnapshot, PublicationGuard)>, VerbalixError> {
        let state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        let snapshot = match &*state {
            SelectionState::Candidate(snapshot) if snapshot.id == id => snapshot.clone(),
            SelectionState::Candidate(snapshot) => {
                diagnostics::coordinator("debounce_ignored_id", Some(snapshot));
                return Ok(None);
            }
            _ => {
                diagnostics::coordinator("debounce_ignored_state", None);
                return Ok(None);
            }
        };
        let presentation = self
            .presentation
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)?;
        let guard = presentation
            .as_ref()
            .filter(|presentation| presentation.snapshot.id == id)
            .map(|presentation| presentation.guard.clone())
            .ok_or(VerbalixError::StaleSelection)?;
        Ok(Some((snapshot, guard)))
    }
}
