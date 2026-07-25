use super::macos_ax_actor_state::ActorState;
use crate::{
    application::{MutationReceipt, PublicationGuard},
    domain::{SelectionSnapshot, VerbalixError},
};
use std::{
    sync::{
        mpsc::{self, SyncSender},
        Mutex,
    },
    thread::JoinHandle,
};
use uuid::Uuid;

enum Command {
    Capture(mpsc::Sender<Result<SelectionSnapshot, VerbalixError>>),
    Replace {
        expected: SelectionSnapshot,
        text: String,
        lease: Option<PublicationGuard>,
        response: mpsc::Sender<Result<MutationReceipt, VerbalixError>>,
    },
    Restore {
        expected: SelectionSnapshot,
        transformed: String,
        lease: Option<PublicationGuard>,
        response: mpsc::Sender<Result<MutationReceipt, VerbalixError>>,
    },
    Discard(Uuid),
    Shutdown,
}

pub(super) struct AxActor {
    sender: SyncSender<Command>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl AxActor {
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel(64);
        let worker = std::thread::spawn(move || {
            let mut state = ActorState::new();
            while let Ok(command) = receiver.recv() {
                match command {
                    Command::Capture(response) => {
                        let _ = response.send(state.capture());
                    }
                    Command::Replace {
                        expected,
                        text,
                        lease,
                        response,
                    } => {
                        let _ = response.send(state.replace(expected, text, lease));
                    }
                    Command::Restore {
                        expected,
                        transformed,
                        lease,
                        response,
                    } => {
                        let _ = response.send(state.restore(expected, transformed, lease));
                    }
                    Command::Discard(id) => {
                        state.discard(id);
                    }
                    Command::Shutdown => break,
                }
            }
        });
        Self {
            sender,
            worker: Mutex::new(Some(worker)),
        }
    }

    pub(super) fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        self.request(Command::Capture)
    }

    pub(super) fn replace(
        &self,
        expected: &SelectionSnapshot,
        text: &str,
        lease: Option<PublicationGuard>,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.request(|response| Command::Replace {
            expected: expected.clone(),
            text: text.to_owned(),
            lease,
            response,
        })
    }

    pub(super) fn restore(
        &self,
        expected: &SelectionSnapshot,
        transformed: &str,
        lease: Option<PublicationGuard>,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.request(|response| Command::Restore {
            expected: expected.clone(),
            transformed: transformed.to_owned(),
            lease,
            response,
        })
    }

    pub(super) fn discard(&self, id: Uuid) {
        let _ = self.sender.send(Command::Discard(id));
    }

    fn request<T>(
        &self,
        command: impl FnOnce(mpsc::Sender<Result<T, VerbalixError>>) -> Command,
    ) -> Result<T, VerbalixError> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(command(sender))
            .map_err(|_| VerbalixError::LocalFailure)?;
        receiver.recv().map_err(|_| VerbalixError::LocalFailure)?
    }
}

impl Drop for AxActor {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Shutdown);
        if let Ok(worker) = self.worker.get_mut() {
            if let Some(worker) = worker.take() {
                let _ = worker.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_is_send_sync_without_sending_ax_handles() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AxActor>();
    }
}
