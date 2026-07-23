use crate::domain::VerbalixError;
use serde::Serialize;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteMode {
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
    current: Mutex<Option<NoteResultPayload>>,
}

impl NoteResultState {
    pub fn publish(&self, payload: NoteResultPayload) -> Result<(), VerbalixError> {
        *self
            .current
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)? = Some(payload);
        Ok(())
    }

    pub fn current(&self) -> Result<Option<NoteResultPayload>, VerbalixError> {
        self.current
            .lock()
            .map(|payload| payload.clone())
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

        state.publish(payload.clone()).unwrap();

        assert_eq!(state.current().unwrap(), Some(payload));
    }

    #[test]
    fn returns_the_latest_result_after_a_listener_is_ready() {
        let state = NoteResultState::default();
        state
            .publish(NoteResultPayload {
                mode: NoteMode::Preview,
                request_id: Some(Uuid::new_v4()),
                text: "Preview".to_owned(),
            })
            .unwrap();
        let updated = NoteResultPayload {
            mode: NoteMode::Undo,
            request_id: None,
            text: "Applied".to_owned(),
        };

        state.publish(updated.clone()).unwrap();

        assert_eq!(state.current().unwrap(), Some(updated));
    }
}
