use super::{
    causal_epoch::CausalEpoch,
    macos_ax_actor_observation::{
        ExpectedSelfNotification, ObservedSelectionChange, SelfNotificationSignal,
    },
    macos_ax_actor_state::ActorState,
    macos_element_token::AxElementToken,
};
use crate::{
    application::{MutationProjection, MutationReceipt, PublicationGuard},
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
        receipt: MutationReceipt,
        expected: SelectionSnapshot,
        text: String,
        lease: Option<PublicationGuard>,
        response: mpsc::Sender<Result<MutationReceipt, VerbalixError>>,
    },
    Restore {
        mutation_id: Uuid,
        expected: SelectionSnapshot,
        transformed: String,
        lease: Option<PublicationGuard>,
        response: mpsc::Sender<Result<MutationReceipt, VerbalixError>>,
    },
    Discard(Uuid),
    Reconcile {
        mutation_id: Uuid,
        response: mpsc::Sender<Option<MutationProjection>>,
    },
    ObserveSelectionChange {
        expected: ExpectedSelfNotification,
        target: AxElementToken,
        generation: u64,
        response: mpsc::Sender<ObservedSelectionChange>,
    },
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
    #[cfg(test)]
    TestActorState(Box<dyn FnOnce(&mut ActorState) + Send>),
    Shutdown,
}

pub(super) struct AxActor {
    sender: SyncSender<Command>,
    worker: Mutex<Option<JoinHandle<()>>>,
    epoch: CausalEpoch,
    self_notifications: SelfNotificationSignal,
}

impl AxActor {
    pub(super) fn new() -> Self {
        let epoch = CausalEpoch::default();
        let worker_epoch = epoch.clone();
        let self_notifications = SelfNotificationSignal::default();
        let worker_notifications = self_notifications.clone();
        let (sender, receiver) = mpsc::sync_channel(64);
        let worker = std::thread::spawn(move || {
            let mut state = ActorState::new(worker_epoch.clone(), worker_notifications);
            while let Ok(command) = receiver.recv() {
                match command {
                    Command::Capture(response) => {
                        let _ = response.send(state.capture());
                    }
                    Command::Replace {
                        receipt,
                        expected,
                        text,
                        lease,
                        response,
                    } => {
                        let _ = response.send(state.replace(receipt, expected, text, lease));
                    }
                    Command::Restore {
                        mutation_id,
                        expected,
                        transformed,
                        lease,
                        response,
                    } => {
                        let _ =
                            response.send(state.restore(mutation_id, expected, transformed, lease));
                    }
                    Command::Discard(id) => {
                        state.discard(id);
                    }
                    Command::Reconcile {
                        mutation_id,
                        response,
                    } => {
                        let _ = response.send(state.reconcile(mutation_id));
                    }
                    Command::ObserveSelectionChange {
                        expected,
                        target,
                        generation,
                        response,
                    } => {
                        let _ = response
                            .send(state.observe_selection_change(expected, target, generation));
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
                    #[cfg(test)]
                    Command::TestActorState(test) => test(&mut state),
                    Command::Shutdown => break,
                }
            }
        });
        Self {
            sender,
            worker: Mutex::new(Some(worker)),
            epoch,
            self_notifications,
        }
    }

    pub(super) fn signal_causal_change(&self) {
        self.epoch.bump();
    }

    pub(super) fn causal_epoch(&self) -> CausalEpoch {
        self.epoch.clone()
    }

    pub(super) fn has_pending_self_notification(&self) -> bool {
        self.self_notifications.has_pending()
    }

    pub(super) fn observe_selection_change(
        &self,
        target: AxElementToken,
        generation: u64,
    ) -> Result<ObservedSelectionChange, VerbalixError> {
        let Some(expected) = self
            .self_notifications
            .take_exact(target.clone(), generation)
        else {
            return Ok(ObservedSelectionChange::External);
        };
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(Command::ObserveSelectionChange {
                expected,
                target,
                generation,
                response: sender,
            })
            .map_err(|_| VerbalixError::LocalFailure)?;
        receiver.recv().map_err(|_| VerbalixError::LocalFailure)
    }

    pub(super) fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        self.request(Command::Capture)
    }

    pub(super) fn replace(
        &self,
        expected: &SelectionSnapshot,
        text: &str,
        lease: Option<PublicationGuard>,
        mutation_id: Uuid,
    ) -> Result<MutationReceipt, VerbalixError> {
        let receipt = MutationReceipt {
            id: mutation_id,
            snapshot_id: expected.id,
            request_id: lease
                .as_ref()
                .map_or(Uuid::nil(), |lease| lease.request_id()),
        };
        self.request(|response| Command::Replace {
            receipt,
            expected: expected.clone(),
            text: text.to_owned(),
            lease,
            response,
        })
    }

    pub(super) fn restore(
        &self,
        mutation_id: Uuid,
        expected: &SelectionSnapshot,
        transformed: &str,
        lease: Option<PublicationGuard>,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.request(|response| Command::Restore {
            mutation_id,
            expected: expected.clone(),
            transformed: transformed.to_owned(),
            lease,
            response,
        })
    }

    pub(super) fn discard(&self, id: Uuid) {
        let _ = self.sender.send(Command::Discard(id));
    }

    pub(super) fn reconcile(
        &self,
        mutation_id: Uuid,
    ) -> Result<Option<MutationProjection>, VerbalixError> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(Command::Reconcile {
                mutation_id,
                response: sender,
            })
            .map_err(|_| VerbalixError::LocalFailure)?;
        receiver.recv().map_err(|_| VerbalixError::LocalFailure)
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
#[path = "macos_ax_actor_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "macos_ax_actor_restore_epoch_tests.rs"]
mod restore_epoch_tests;

#[cfg(test)]
#[path = "macos_ax_actor_notification_phase_tests.rs"]
mod notification_phase_tests;
