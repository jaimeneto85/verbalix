use super::*;
use crate::{
    application::{MutationReceipt, MutationStatus, TransformLease},
    domain::{Rect, SelectionElementIdentity, SelectionExtractionStrategy, TextRange},
    platform::{
        macos_accessibility::route_observer_event,
        macos_ax_actor_state::CapturedTarget,
        macos_ax_target::AxMutationTarget,
        macos_classic_range::CFRange,
        macos_mutation_ledger::{ReplaceTerminalOutcome, TerminalPhase},
        macos_observer::{AccessibilityEvent, AccessibilityEventKind},
        macos_replace::WriteOutcome,
        macos_restore::RestoreWriteOutcome,
        macos_selection_revalidation::CurrentSelection,
    },
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

struct BlockingRestoreTarget {
    entered: mpsc::Sender<()>,
    release: RefCell<Option<mpsc::Receiver<()>>>,
    setters: Arc<AtomicUsize>,
}

impl AxMutationTarget for BlockingRestoreTarget {
    fn prepare_replace(
        &self,
        _expected: &SelectionSnapshot,
        _causal: bool,
    ) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn write_replace(&self, _expected: &SelectionSnapshot, _text: &str) -> WriteOutcome {
        WriteOutcome::Confirmed
    }

    fn prepare_restore(
        &self,
        _expected: &SelectionSnapshot,
        _transformed: &str,
        _causal: bool,
    ) -> Result<(), VerbalixError> {
        let _ = self.entered.send(());
        self.release
            .borrow_mut()
            .take()
            .ok_or(VerbalixError::LocalFailure)?
            .recv()
            .map_err(|_| VerbalixError::LocalFailure)
    }

    fn write_restore(&self, _expected: &SelectionSnapshot) -> RestoreWriteOutcome {
        self.setters.fetch_add(1, Ordering::SeqCst);
        RestoreWriteOutcome::Confirmed
    }

    fn read(
        &self,
        _strategy: SelectionExtractionStrategy,
    ) -> Result<CurrentSelection, VerbalixError> {
        Ok(CurrentSelection {
            text: "before".to_owned(),
            range: CFRange {
                location: 0,
                length: 6,
            },
            strategy: SelectionExtractionStrategy::SelectedText,
        })
    }
}

fn restore_snapshot() -> SelectionSnapshot {
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

#[test]
fn focus_epoch_rejects_real_restore_before_pending_capture_and_preserves_ledger() {
    let actor = AxActor::new();
    let epoch = actor.epoch.current();
    let expected = restore_snapshot();
    let request_id = Uuid::new_v4();
    let receipt = MutationReceipt {
        id: Uuid::new_v4(),
        snapshot_id: expected.id,
        request_id,
    };
    let lease = Arc::new(TransformLease::new(expected.id, request_id));
    let lease_probe = lease.clone();
    let setters = Arc::new(AtomicUsize::new(0));
    let setters_worker = setters.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let expected_worker = expected.clone();
    let receipt_worker = receipt.clone();
    actor
        .sender
        .send(Command::TestActorState(Box::new(move |state| {
            let target = CapturedTarget {
                target: Rc::new(BlockingRestoreTarget {
                    entered: entered_tx,
                    release: RefCell::new(Some(release_rx)),
                    setters: setters_worker,
                }),
                epoch,
                token: AxElementToken::new(expected_worker.pid, "editor"),
            };
            state
                .targets
                .insert(expected_worker.id, target.clone(), state.now());
            state
                .mutations
                .prepare(
                    receipt_worker.clone(),
                    expected_worker.clone(),
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
            let before = state.mutations.get_mut(receipt_worker.id).unwrap();
            let before_meta = (
                before.projection.status,
                before.terminal_at,
                before.restore_attempted,
                before.terminal_phase,
            );
            let result = state.restore(
                receipt_worker.id,
                expected_worker,
                "after".to_owned(),
                Some(lease),
            );
            let after = state.mutations.get_mut(receipt_worker.id).unwrap();
            let after_meta = (
                after.projection.status,
                after.terminal_at,
                after.restore_attempted,
                after.terminal_phase,
            );
            let _ = result_tx.send((result, before_meta, after_meta));
        })))
        .unwrap();
    entered_rx.recv().unwrap();

    assert!(route_observer_event(
        AccessibilityEvent {
            kind: AccessibilityEventKind::FocusChanged,
            target: Some(AxElementToken::new(42, "other").unwrap()),
        },
        &actor.causal_epoch(),
        |_, _| panic!("focus must bump outside the actor FIFO"),
    ));
    let (capture_tx, capture_rx) = mpsc::channel();
    actor
        .sender
        .send(Command::TestActorState(Box::new(move |_| {
            let _ = capture_tx.send(());
        })))
        .unwrap();
    assert!(matches!(
        capture_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    release_tx.send(()).unwrap();

    let (result, before, after) = result_rx.recv().unwrap();
    assert!(matches!(result, Err(VerbalixError::StaleSelection)));
    assert!(before.0 == MutationStatus::Confirmed);
    assert!(after.0 == MutationStatus::Confirmed);
    assert_eq!(before.1, after.1);
    assert_eq!(before.2, after.2);
    assert_eq!(before.3, after.3);
    assert_eq!(before.3, Some(TerminalPhase::FinishReplace));
    assert_eq!(setters.load(Ordering::SeqCst), 0);
    assert!(
        lease_probe.try_claim_write(),
        "stale epoch must reject restore before consuming the write lease"
    );
    capture_rx.recv().unwrap();
}
