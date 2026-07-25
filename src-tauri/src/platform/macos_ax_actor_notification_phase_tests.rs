use super::*;
use crate::{
    application::{MutationStatus, TransformLease},
    domain::{
        Rect, SelectionElementIdentity, SelectionExtractionStrategy, SelectionSnapshot, TextRange,
    },
    platform::{
        macos_accessibility::route_observer_event,
        macos_ax_actor_state::CapturedTarget,
        macos_ax_target::AxMutationTarget,
        macos_mutation_ledger::{ReplaceTerminalOutcome, TerminalPhase},
        macos_observer::{AccessibilityEvent, AccessibilityEventKind},
        macos_replace::WriteOutcome,
        macos_restore::RestoreWriteOutcome,
        macos_selection_revalidation::CurrentSelection,
        macos_write_authorization::AxWriteAuthorization,
    },
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
};

#[derive(Clone, Copy)]
enum MutationKind {
    Replace,
    Restore,
}

struct ArmedBlockingTarget {
    entered: mpsc::Sender<()>,
    release: RefCell<Option<mpsc::Receiver<()>>>,
    setters: Arc<AtomicUsize>,
}

impl ArmedBlockingTarget {
    fn authorize(&self, authorization: &AxWriteAuthorization) -> bool {
        let _ = self.entered.send(());
        let _ = self.release.borrow_mut().take().unwrap().recv();
        if authorization.begin_setter() {
            self.setters.fetch_add(1, Ordering::SeqCst);
            authorization.commit();
            true
        } else {
            false
        }
    }
}

impl AxMutationTarget for ArmedBlockingTarget {
    fn prepare_replace(
        &self,
        _expected: &SelectionSnapshot,
        _causal: bool,
    ) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn write_replace(
        &self,
        _expected: &SelectionSnapshot,
        _text: &str,
        authorization: &AxWriteAuthorization,
    ) -> WriteOutcome {
        if self.authorize(authorization) {
            WriteOutcome::Confirmed
        } else {
            WriteOutcome::Rejected
        }
    }

    fn prepare_restore(
        &self,
        _expected: &SelectionSnapshot,
        _transformed: &str,
        _causal: bool,
    ) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn write_restore(
        &self,
        _expected: &SelectionSnapshot,
        authorization: &AxWriteAuthorization,
    ) -> RestoreWriteOutcome {
        if self.authorize(authorization) {
            RestoreWriteOutcome::Confirmed
        } else {
            RestoreWriteOutcome::Rejected
        }
    }

    fn read(
        &self,
        _strategy: SelectionExtractionStrategy,
    ) -> Result<CurrentSelection, VerbalixError> {
        Err(VerbalixError::LocalFailure)
    }
}

fn snapshot() -> SelectionSnapshot {
    SelectionSnapshot::new(
        42,
        "pid:42".to_owned(),
        "before".to_owned(),
        TextRange {
            location: 0,
            length: 6,
        },
        Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        true,
    )
    .with_element_identity(SelectionElementIdentity {
        role: "AXTextField".to_owned(),
        subrole: None,
        frame: Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
    })
    .with_native_element_identifier(Some("editor".to_owned()))
}

fn assert_exact_external_event_revokes_armed_write(kind: MutationKind) {
    let actor = AxActor::new();
    let selected = snapshot();
    let epoch = actor.epoch.current();
    let request_id = Uuid::new_v4();
    let receipt = MutationReceipt {
        id: Uuid::new_v4(),
        snapshot_id: selected.id,
        request_id,
    };
    let lease = Arc::new(TransformLease::new(selected.id, request_id));
    let lease_probe = lease.clone();
    let setters = Arc::new(AtomicUsize::new(0));
    let setters_worker = setters.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let selected_worker = selected.clone();
    let receipt_worker = receipt.clone();
    actor
        .sender
        .send(Command::TestActorState(Box::new(move |state| {
            let target = CapturedTarget {
                target: Rc::new(ArmedBlockingTarget {
                    entered: entered_tx,
                    release: RefCell::new(Some(release_rx)),
                    setters: setters_worker,
                }),
                epoch,
                token: AxElementToken::new(selected_worker.pid, "editor"),
            };
            state
                .targets
                .insert(selected_worker.id, target.clone(), state.now());
            if matches!(kind, MutationKind::Restore) {
                state
                    .mutations
                    .prepare(
                        receipt_worker.clone(),
                        selected_worker.clone(),
                        "after".to_owned(),
                        target,
                        state.now(),
                    )
                    .unwrap();
                state
                    .mutations
                    .finish_replace(
                        receipt_worker.id,
                        ReplaceTerminalOutcome::Confirmed,
                        state.now(),
                    )
                    .unwrap();
            }
            let result = match kind {
                MutationKind::Replace => state.replace(
                    receipt_worker.clone(),
                    selected_worker,
                    "after".to_owned(),
                    Some(lease),
                ),
                MutationKind::Restore => state.restore(
                    receipt_worker.id,
                    selected_worker,
                    "after".to_owned(),
                    Some(lease),
                ),
            };
            let now = state.now();
            let record = state.mutations.get_mut(receipt_worker.id, now).unwrap();
            let _ = result_tx.send((
                result,
                record.projection.status,
                record.terminal_phase,
                state.self_notifications.has_pending(),
            ));
        })))
        .unwrap();
    entered_rx.recv().unwrap();
    assert!(actor.has_pending_self_notification());

    let (capture_tx, capture_rx) = mpsc::channel();
    actor
        .sender
        .send(Command::TestPendingCapture(capture_tx))
        .unwrap();
    assert!(matches!(
        capture_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    let generation = actor.epoch.current();
    let routed = route_observer_event(
        AccessibilityEvent {
            kind: AccessibilityEventKind::SelectedTextChanged,
            target: Some(AxElementToken::new(selected.pid, "editor").unwrap()),
        },
        &actor.causal_epoch(),
        |target, generation| actor.observe_selection_change(target, generation),
    );

    assert!(routed);
    assert!(!actor.epoch.is_current(generation));
    assert!(!actor.has_pending_self_notification());
    release_tx.send(()).unwrap();
    let (result, status, phase, pending) = result_rx.recv().unwrap();
    assert!(matches!(result, Err(VerbalixError::LocalFailure)));
    assert_eq!(setters.load(Ordering::SeqCst), 0);
    assert!(!lease_probe.try_claim_write());
    assert!(!pending);
    match kind {
        MutationKind::Replace => {
            assert!(status == MutationStatus::Rejected);
            assert_eq!(phase, Some(TerminalPhase::FinishReplace));
        }
        MutationKind::Restore => {
            assert!(status == MutationStatus::RestoreRejected);
            assert_eq!(phase, Some(TerminalPhase::FinishRestore));
        }
    }
    capture_rx.recv().unwrap();
}

#[test]
fn exact_external_selection_revokes_armed_replace_outside_actor_fifo() {
    assert_exact_external_event_revokes_armed_write(MutationKind::Replace);
}

#[test]
fn exact_external_selection_revokes_armed_restore_outside_actor_fifo() {
    assert_exact_external_event_revokes_armed_write(MutationKind::Restore);
}
