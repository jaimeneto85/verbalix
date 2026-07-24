use crate::{application::PublicationGuard, domain::VerbalixError};
use serde::Serialize;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteMode {
    Error,
    Result,
    Preview,
    Undo,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteResultPayload {
    pub mode: NoteMode,
    pub request_id: Option<Uuid>,
    pub text: String,
}

#[derive(Default)]
pub struct NoteResultState {
    current: Mutex<Option<StoredNoteResult>>,
}

struct StoredNoteResult {
    payload: NoteResultPayload,
    guard: Option<PublicationGuard>,
}

impl NoteResultState {
    pub fn publish(
        &self,
        payload: NoteResultPayload,
        guard: Option<PublicationGuard>,
    ) -> Result<(), VerbalixError> {
        *self
            .current
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)? = Some(StoredNoteResult { payload, guard });
        Ok(())
    }

    pub fn current(&self) -> Result<Option<NoteResultPayload>, VerbalixError> {
        self.current
            .lock()
            .map(|current| {
                current.as_ref().and_then(|current| {
                    current
                        .guard
                        .as_ref()
                        .is_none_or(|guard| guard.may_publish())
                        .then(|| current.payload.clone())
                })
            })
            .map_err(|_| VerbalixError::LocalFailure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_a_result_published_before_a_listener() {
        let state = NoteResultState::default();
        let payload = NoteResultPayload {
            mode: NoteMode::Result,
            request_id: None,
            text: "Translated".to_owned(),
        };

        state.publish(payload.clone(), None).unwrap();

        assert_eq!(state.current().unwrap(), Some(payload));
    }

    #[test]
    fn returns_the_latest_result_after_a_listener_is_ready() {
        let state = NoteResultState::default();
        state
            .publish(
                NoteResultPayload {
                    mode: NoteMode::Preview,
                    request_id: Some(Uuid::new_v4()),
                    text: "Preview".to_owned(),
                },
                None,
            )
            .unwrap();
        let updated = NoteResultPayload {
            mode: NoteMode::Error,
            request_id: None,
            text: "Entre no Verbalix para continuar.".to_owned(),
        };

        state.publish(updated.clone(), None).unwrap();

        assert_eq!(state.current().unwrap(), Some(updated));
    }
}
