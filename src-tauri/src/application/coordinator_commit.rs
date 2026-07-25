use super::SelectionCoordinator;
use crate::{
    application::PublicationGuard,
    domain::{SelectionSnapshot, SelectionState, VerbalixError},
};
use uuid::Uuid;

impl SelectionCoordinator {
    pub(super) fn active_processing(
        &self,
        snapshot: &SelectionSnapshot,
        request_id: Uuid,
    ) -> Result<(SelectionSnapshot, PublicationGuard), VerbalixError> {
        let state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        let current = match &*state {
            SelectionState::Processing {
                snapshot: current,
                request_id: active_request,
            } if *active_request == request_id && current.same_target(snapshot) => current.clone(),
            _ => return Err(VerbalixError::StaleSelection),
        };
        let active = self
            .active_transform
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)?;
        let lease = active
            .as_ref()
            .filter(|active| {
                active.request_id == request_id && active.snapshot.same_target(&current)
            })
            .map(|active| active.lease.clone())
            .ok_or(VerbalixError::StaleSelection)?;
        Ok((current, lease))
    }

    pub(super) fn commit_result_visible(
        &self,
        snapshot: &SelectionSnapshot,
        request_id: Uuid,
        lease: &PublicationGuard,
    ) -> Result<bool, VerbalixError> {
        self.commit_if_current(snapshot, request_id, lease, SelectionState::ResultVisible)
    }

    pub(super) fn commit_preview_visible(
        &self,
        snapshot: &SelectionSnapshot,
        request_id: Uuid,
        result: &str,
        lease: &PublicationGuard,
    ) -> Result<bool, VerbalixError> {
        let result = result.to_owned();
        self.commit_if_current(snapshot, request_id, lease, move |snapshot| {
            SelectionState::PreviewVisible {
                snapshot,
                request_id,
                result,
            }
        })
    }

    pub(super) fn commit_applied(
        &self,
        snapshot: &SelectionSnapshot,
        result: &str,
        lease: &PublicationGuard,
        mutation: &super::mutation_journal::MutationRecord,
    ) -> Result<Option<PublicationGuard>, VerbalixError> {
        let request_id = mutation.receipt.request_id;
        let mut state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        let state_matches = matches!(
            &*state,
            SelectionState::Processing {
                snapshot: current,
                request_id: active_request,
            } | SelectionState::PreviewVisible {
                snapshot: current,
                request_id: active_request,
                ..
            } if *active_request == request_id && current.same_target(snapshot)
        );
        let mut active = self
            .active_transform
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)?;
        let active_matches = active.as_ref().is_some_and(|active| {
            active.request_id == request_id
                && active.snapshot.same_target(snapshot)
                && std::sync::Arc::ptr_eq(&active.lease, lease)
        });
        if !state_matches || !active_matches || !lease.may_publish() {
            return Ok(None);
        }
        lease.cancel();
        let undo_lease = mutation.undo_lease.clone();
        *active = Some(super::coordinator::ActiveTransform {
            snapshot: snapshot.clone(),
            request_id,
            lease: undo_lease.clone(),
        });
        *state = SelectionState::Applied {
            snapshot: snapshot.clone(),
            transformed_text: result.to_owned(),
            mutation_id: mutation.receipt.id,
        };
        Ok(Some(undo_lease))
    }

    pub(super) fn commit_undo(
        &self,
        snapshot: &SelectionSnapshot,
        transformed_text: &str,
        lease: &PublicationGuard,
    ) -> Result<bool, VerbalixError> {
        let mut state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        let state_matches = matches!(
            &*state,
            SelectionState::Applied {
                snapshot: current,
                transformed_text: active,
                ..
            } if active == transformed_text && current.same_target(snapshot)
        );
        let mut active = self
            .active_transform
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)?;
        let active_matches = active.as_ref().is_some_and(|active| {
            active.snapshot.same_target(snapshot) && std::sync::Arc::ptr_eq(&active.lease, lease)
        });
        if !state_matches || !active_matches {
            return Ok(false);
        }
        lease.cancel();
        *active = None;
        *state = SelectionState::Idle;
        Ok(true)
    }

    fn commit_if_current(
        &self,
        snapshot: &SelectionSnapshot,
        request_id: Uuid,
        lease: &PublicationGuard,
        next: impl FnOnce(SelectionSnapshot) -> SelectionState,
    ) -> Result<bool, VerbalixError> {
        let mut state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        let state_matches = matches!(
            &*state,
            SelectionState::Processing {
                snapshot: current,
                request_id: active_request,
            } if *active_request == request_id && current.same_target(snapshot)
        ) || matches!(
            &*state,
            SelectionState::PreviewVisible {
                snapshot: current,
                request_id: active_request,
                ..
            } if *active_request == request_id && current.same_target(snapshot)
        );
        let active = self
            .active_transform
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)?;
        let active_matches = active.as_ref().is_some_and(|active| {
            active.request_id == request_id
                && active.snapshot.same_target(snapshot)
                && std::sync::Arc::ptr_eq(&active.lease, lease)
        });
        if !state_matches || !active_matches || !lease.may_publish() {
            return Ok(false);
        }
        *state = next(snapshot.clone());
        Ok(true)
    }
}
