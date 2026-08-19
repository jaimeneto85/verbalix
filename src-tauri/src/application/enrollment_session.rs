use crate::domain::{EnrollmentSample, VerbalixError};
use std::sync::Mutex;

#[derive(Default)]
enum SessionState {
    #[default]
    Idle,
    Recording,
    Done(EnrollmentSample),
    #[allow(dead_code)]
    Failed,
}

pub struct EnrollmentSession {
    state: Mutex<SessionState>,
}

impl EnrollmentSession {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(SessionState::Idle),
        }
    }

    pub fn begin(&self) -> Result<(), VerbalixError> {
        let mut guard = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        match &*guard {
            SessionState::Idle | SessionState::Done(_) | SessionState::Failed => {
                *guard = SessionState::Recording;
                Ok(())
            }
            SessionState::Recording => Err(VerbalixError::OperationInProgress),
        }
    }

    pub fn finish(&self, sample: EnrollmentSample) -> Result<(), VerbalixError> {
        let mut guard = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        match &*guard {
            SessionState::Recording => {
                *guard = SessionState::Done(sample);
                Ok(())
            }
            _ => Err(VerbalixError::LocalFailure),
        }
    }

    pub fn take_sample(&self) -> Result<EnrollmentSample, VerbalixError> {
        let mut guard = self.state.lock().map_err(|_| VerbalixError::LocalFailure)?;
        match std::mem::replace(&mut *guard, SessionState::Idle) {
            SessionState::Done(sample) => Ok(sample),
            other => {
                *guard = other;
                Err(VerbalixError::LocalFailure)
            }
        }
    }

    pub fn cancel(&self) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = SessionState::Idle;
        }
    }

    #[allow(dead_code)]
    pub fn is_recording(&self) -> bool {
        matches!(
            &*self.state.lock().unwrap_or_else(|p| p.into_inner()),
            SessionState::Recording
        )
    }
}
