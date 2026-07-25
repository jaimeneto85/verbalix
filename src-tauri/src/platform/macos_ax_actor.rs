use super::{causal_epoch::CausalEpoch, macos_ax_actor_state::ActorState};
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
    #[cfg(test)]
    TestBoundary {
        observed_epoch: u64,
        lease: PublicationGuard,
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
        setters: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        response: mpsc::Sender<Result<(), VerbalixError>>,
    },
    #[cfg(test)]
    TestPendingCapture(mpsc::Sender<()>),
    Shutdown,
}

pub(super) struct AxActor {
    sender: SyncSender<Command>,
    worker: Mutex<Option<JoinHandle<()>>>,
    epoch: CausalEpoch,
}

impl AxActor {
    pub(super) fn new() -> Self {
        let epoch = CausalEpoch::default();
        let worker_epoch = epoch.clone();
        let (sender, receiver) = mpsc::sync_channel(64);
        let worker = std::thread::spawn(move || {
            let mut state = ActorState::new(worker_epoch.clone());
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
                    #[cfg(test)]
                    Command::TestBoundary {
                        observed_epoch,
                        lease,
                        entered,
                        release,
                        setters,
                        response,
                    } => {
                        let _ = entered.send(());
                        let _ = release.recv();
                        let result = if worker_epoch.is_current(observed_epoch)
                            && lease.try_claim_write()
                            && worker_epoch.is_current(observed_epoch)
                        {
                            setters.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            Ok(())
                        } else {
                            Err(VerbalixError::StaleSelection)
                        };
                        let _ = response.send(result);
                    }
                    #[cfg(test)]
                    Command::TestPendingCapture(response) => {
                        let _ = response.send(());
                    }
                    Command::Shutdown => break,
                }
            }
        });
        Self {
            sender,
            worker: Mutex::new(Some(worker)),
            epoch,
        }
    }

    pub(super) fn signal_causal_change(&self) {
        self.epoch.bump();
    }

    pub(super) fn causal_epoch(&self) -> CausalEpoch {
        self.epoch.clone()
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
    use crate::application::TransformLease;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn actor_is_send_sync_without_sending_ax_handles() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AxActor>();
    }

    #[test]
    fn external_epoch_revokes_blocked_write_before_pending_capture_runs() {
        let actor = AxActor::new();
        let observed_epoch = actor.epoch.current();
        let lease = std::sync::Arc::new(TransformLease::new(Uuid::new_v4(), Uuid::new_v4()));
        let setters = std::sync::Arc::new(AtomicUsize::new(0));
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        actor
            .sender
            .send(Command::TestBoundary {
                observed_epoch,
                lease,
                entered: entered_tx,
                release: release_rx,
                setters: setters.clone(),
                response: result_tx,
            })
            .unwrap();
        entered_rx.recv().unwrap();

        actor.signal_causal_change();
        let (capture_tx, capture_rx) = mpsc::channel();
        actor
            .sender
            .send(Command::TestPendingCapture(capture_tx))
            .unwrap();
        release_tx.send(()).unwrap();

        assert!(matches!(
            result_rx.recv().unwrap(),
            Err(VerbalixError::StaleSelection)
        ));
        capture_rx.recv().unwrap();
        assert_eq!(setters.load(Ordering::SeqCst), 0);
    }
}
