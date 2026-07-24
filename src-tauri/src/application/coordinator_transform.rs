use super::SelectionCoordinator;
use crate::{
    diagnostics,
    domain::{SelectionSnapshot, SelectionState, TransformRequest, TransformResult, VerbalixError},
};
use uuid::Uuid;

impl SelectionCoordinator {
    pub fn begin_transform(
        &self,
        snapshot_id: Uuid,
        request_id: Uuid,
    ) -> Result<SelectionSnapshot, VerbalixError> {
        let mut state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        if matches!(&*state, SelectionState::Processing { .. }) {
            return Err(VerbalixError::OperationInProgress);
        }
        let snapshot = match &*state {
            SelectionState::ToolbarVisible(snapshot)
            | SelectionState::ResultVisible(snapshot)
            | SelectionState::PreviewVisible { snapshot, .. }
                if snapshot.id == snapshot_id =>
            {
                snapshot.clone()
            }
            _ => return Err(VerbalixError::StaleSelection),
        };
        self.cancel_presentation()?;
        self.install_active_transform(snapshot.clone(), request_id)?;
        *state = SelectionState::Processing {
            snapshot: snapshot.clone(),
            request_id,
        };
        diagnostics::coordinator("transform_pinned", Some(&snapshot));
        Ok(snapshot)
    }

    pub async fn transform(
        &self,
        snapshot_id: Uuid,
        request: TransformRequest,
        access_token: &str,
        preview_writable: bool,
    ) -> Result<TransformResult, VerbalixError> {
        request.validate()?;
        let snapshot = self
            .processing_snapshot(snapshot_id, request.request_id)?
            .filter(|snapshot| snapshot.text == request.text)
            .ok_or(VerbalixError::StaleSelection)?;
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
        if response.request_id != request.request_id || response.result.trim().is_empty() {
            return Err(VerbalixError::InvalidResponse);
        }
        let (active, lease) = self.active_processing(snapshot, request.request_id)?;
        if !active.writable {
            self.overlay
                .show_note_guarded(active.bounds, &response.result, lease.clone())?;
            self.commit_result_visible(&active, request.request_id, &lease)?;
            return Ok(());
        }
        if preview_writable {
            self.overlay.show_preview_guarded(
                active.bounds,
                request.request_id,
                &response.result,
                lease.clone(),
            )?;
            self.commit_preview_visible(&active, request.request_id, &response.result, &lease)?;
            return Ok(());
        }
        self.selection
            .replace_guarded(&active, &response.result, &lease)?;
        let Some(undo_lease) =
            self.commit_applied(&active, request.request_id, &response.result, &lease)?
        else {
            return Ok(());
        };
        if self
            .overlay
            .show_undo_guarded(active.bounds, &response.result, undo_lease)
            .is_err()
        {
            diagnostics::coordinator("undo_feedback_failed_after_write", Some(&active));
        }
        Ok(())
    }

    pub fn apply_preview(&self, request_id: Uuid) -> Result<String, VerbalixError> {
        let (snapshot, result, lease) = {
            let state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
            let (snapshot, result) = match &*state {
                SelectionState::PreviewVisible {
                    snapshot,
                    request_id: active,
                    result,
                } if *active == request_id => (snapshot.clone(), result.clone()),
                _ => return Err(VerbalixError::StaleSelection),
            };
            let active = self
                .active_transform
                .lock()
                .map_err(|_| VerbalixError::LocalFailure)?;
            let lease = active
                .as_ref()
                .filter(|active| {
                    active.request_id == request_id && active.snapshot.same_target(&snapshot)
                })
                .map(|active| active.lease.clone())
                .ok_or(VerbalixError::StaleSelection)?;
            (snapshot, result, lease)
        };
        self.selection.replace_guarded(&snapshot, &result, &lease)?;
        let Some(undo_lease) = self.commit_applied(&snapshot, request_id, &result, &lease)? else {
            return Ok(result);
        };
        if self
            .overlay
            .show_undo_guarded(snapshot.bounds, &result, undo_lease)
            .is_err()
        {
            diagnostics::coordinator("undo_feedback_failed_after_write", Some(&snapshot));
        }
        Ok(result)
    }

    pub fn undo(&self, transformed_text: &str) -> Result<(), VerbalixError> {
        let (snapshot, lease) = {
            let state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
            let snapshot = match &*state {
                SelectionState::Applied {
                    snapshot,
                    transformed_text: active,
                } if active == transformed_text => snapshot.clone(),
                _ => return Err(VerbalixError::StaleSelection),
            };
            let active = self
                .active_transform
                .lock()
                .map_err(|_| VerbalixError::LocalFailure)?;
            let lease = active
                .as_ref()
                .filter(|active| active.snapshot.same_target(&snapshot))
                .map(|active| active.lease.clone())
                .ok_or(VerbalixError::StaleSelection)?;
            (snapshot, lease)
        };
        self.selection
            .restore_guarded(&snapshot, transformed_text, &lease)?;
        if self.commit_undo(&snapshot, transformed_text, &lease)?
            && self.overlay.hide_all().is_err()
        {
            diagnostics::coordinator("undo_hide_failed_after_restore", Some(&snapshot));
        }
        Ok(())
    }

    pub(crate) fn preview_feedback(
        &self,
        request_id: Uuid,
    ) -> Result<Option<(crate::domain::Rect, crate::application::PublicationGuard)>, VerbalixError>
    {
        let state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        let snapshot = match &*state {
            SelectionState::PreviewVisible {
                snapshot,
                request_id: active,
                ..
            } if *active == request_id => snapshot,
            _ => return Ok(None),
        };
        let active = self
            .active_transform
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)?;
        Ok(active
            .as_ref()
            .filter(|active| active.request_id == request_id)
            .map(|active| (snapshot.bounds, active.lease.clone())))
    }

    pub(crate) fn undo_feedback(
        &self,
        transformed_text: &str,
    ) -> Result<Option<(crate::domain::Rect, crate::application::PublicationGuard)>, VerbalixError>
    {
        let state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        let snapshot = match &*state {
            SelectionState::Applied {
                snapshot,
                transformed_text: active,
            } if active == transformed_text => snapshot,
            _ => return Ok(None),
        };
        let active = self
            .active_transform
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)?;
        Ok(active
            .as_ref()
            .filter(|active| active.snapshot.same_target(snapshot))
            .map(|active| (snapshot.bounds, active.lease.clone())))
    }

    fn fail<T>(&self, request_id: Uuid, error: VerbalixError) -> Result<T, VerbalixError> {
        self.recover_request(request_id)?;
        Err(error)
    }

    pub fn abort_transform(&self, request_id: Uuid) -> Result<(), VerbalixError> {
        self.recover_request(request_id)
    }

    fn processing_snapshot(
        &self,
        snapshot_id: Uuid,
        request_id: Uuid,
    ) -> Result<Option<SelectionSnapshot>, VerbalixError> {
        let state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        Ok(match &*state {
            SelectionState::Processing {
                snapshot,
                request_id: active,
            } if snapshot.id == snapshot_id && *active == request_id => Some(snapshot.clone()),
            _ => None,
        })
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
}
