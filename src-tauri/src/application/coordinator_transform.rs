use super::SelectionCoordinator;
use crate::{
    diagnostics,
    domain::{
        SelectionEvent, SelectionSnapshot, SelectionState, TransformRequest, TransformResult,
        VerbalixError,
    },
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
        let mut state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        let active = match &*state {
            SelectionState::Processing {
                snapshot: current,
                request_id: active_request,
            } if *active_request == request.request_id && current.same_target(snapshot) => {
                current.clone()
            }
            _ => return Err(VerbalixError::StaleSelection),
        };
        if !active.writable {
            self.overlay.show_note(active.bounds, &response.result)?;
            *state = SelectionState::ResultVisible(active);
            return Ok(());
        }
        if preview_writable {
            self.overlay
                .show_preview(active.bounds, request.request_id, &response.result)?;
            *state = SelectionState::PreviewVisible {
                snapshot: active,
                request_id: request.request_id,
                result: response.result.clone(),
            };
            return Ok(());
        }
        self.selection.replace(&active, &response.result)?;
        *state = SelectionState::Applied {
            snapshot: active.clone(),
            transformed_text: response.result.clone(),
        };
        if self
            .overlay
            .show_undo(active.bounds, &response.result)
            .is_err()
        {
            diagnostics::coordinator("undo_feedback_failed_after_write", Some(&active));
        }
        Ok(())
    }

    pub fn apply_preview(&self, request_id: Uuid) -> Result<String, VerbalixError> {
        let mut state = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        let (snapshot, result) = match &*state {
            SelectionState::PreviewVisible {
                snapshot,
                request_id: active,
                result,
            } if *active == request_id => (snapshot.clone(), result.clone()),
            _ => return Err(VerbalixError::StaleSelection),
        };
        self.selection.replace(&snapshot, &result)?;
        *state = SelectionState::Applied {
            snapshot: snapshot.clone(),
            transformed_text: result.clone(),
        };
        if self.overlay.show_undo(snapshot.bounds, &result).is_err() {
            diagnostics::coordinator("undo_feedback_failed_after_write", Some(&snapshot));
        }
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
