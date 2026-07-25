use super::*;
use crate::{
    application::{MutationStatus, TransformLease},
    domain::{Rect, SelectionElementIdentity, SelectionExtractionStrategy, TextRange},
    platform::{
        macos_classic_range::CFRange,
        macos_mutation_ledger::{ReplaceTerminalOutcome, RestoreTerminalOutcome, TerminalPhase},
        macos_replace::WriteOutcome,
        macos_restore::RestoreWriteOutcome,
        macos_selection_revalidation::CurrentSelection,
    },
};
use std::{cell::Cell, sync::Arc};

struct RestoreTarget {
    reads: Cell<usize>,
}

impl RestoreTarget {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            reads: Cell::new(0),
        })
    }
}

impl AxMutationTarget for RestoreTarget {
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
        if authorization.begin_setter() {
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
        if authorization.begin_setter() {
            RestoreWriteOutcome::Confirmed
        } else {
            RestoreWriteOutcome::Rejected
        }
    }

    fn read(
        &self,
        strategy: SelectionExtractionStrategy,
    ) -> Result<CurrentSelection, VerbalixError> {
        self.reads.set(self.reads.get() + 1);
        Ok(CurrentSelection {
            text: "before".to_owned(),
            range: CFRange {
                location: 0,
                length: 6,
            },
            strategy,
        })
    }
}

#[derive(Clone, Copy)]
struct RecordMeta {
    status: MutationStatus,
    terminal_at: Option<u64>,
    restore_attempted: bool,
    terminal_phase: Option<TerminalPhase>,
}

#[derive(Clone, Copy)]
enum Divergence {
    MutationId,
    SnapshotId,
    SnapshotText,
    Transformed,
    LeaseSnapshot,
    LeaseRequest,
}

fn selected() -> SelectionSnapshot {
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

fn terminal_state(
    outcome: RestoreTerminalOutcome,
) -> (
    ActorState,
    SelectionSnapshot,
    MutationReceipt,
    Arc<TransformLease>,
    Rc<RestoreTarget>,
) {
    let epoch = CausalEpoch::default();
    let mut state = ActorState::new(epoch.clone(), SelfNotificationSignal::default());
    let expected = selected();
    let request_id = Uuid::new_v4();
    let receipt = MutationReceipt {
        id: Uuid::new_v4(),
        snapshot_id: expected.id,
        request_id,
    };
    let lease = Arc::new(TransformLease::new(expected.id, request_id));
    let instrumented = RestoreTarget::new();
    let target = CapturedTarget {
        target: instrumented.clone(),
        epoch: epoch.current(),
        token: AxElementToken::new(expected.pid, "editor"),
    };
    state
        .mutations
        .prepare(
            receipt.clone(),
            expected.clone(),
            "after".to_owned(),
            target,
            state.now(),
        )
        .unwrap();
    state
        .mutations
        .finish_replace(receipt.id, ReplaceTerminalOutcome::Confirmed, state.now())
        .unwrap();
    state
        .mutations
        .begin_restore(receipt.id, state.now())
        .unwrap();
    state
        .mutations
        .finish_restore(receipt.id, outcome, state.now())
        .unwrap();
    (state, expected, receipt, lease, instrumented)
}

fn record_meta(state: &mut ActorState, id: Uuid) -> RecordMeta {
    let now = state.now();
    let record = state.mutations.get_mut(id, now).unwrap();
    RecordMeta {
        status: record.projection.status,
        terminal_at: record.terminal_at,
        restore_attempted: record.restore_attempted,
        terminal_phase: record.terminal_phase,
    }
}

fn assert_meta_eq(actual: RecordMeta, expected: RecordMeta) {
    assert!(actual.status == expected.status);
    assert_eq!(actual.terminal_at, expected.terminal_at);
    assert_eq!(actual.restore_attempted, expected.restore_attempted);
    assert_eq!(actual.terminal_phase, expected.terminal_phase);
}

fn assert_divergence_is_inert(outcome: RestoreTerminalOutcome, divergence: Divergence) {
    let (mut state, mut expected, receipt, valid_lease, instrumented) = terminal_state(outcome);
    let before = record_meta(&mut state, receipt.id);
    let mut mutation_id = receipt.id;
    let mut transformed = "after".to_owned();
    let lease = match divergence {
        Divergence::MutationId => {
            mutation_id = Uuid::new_v4();
            valid_lease.clone()
        }
        Divergence::SnapshotId => {
            expected.id = Uuid::new_v4();
            valid_lease.clone()
        }
        Divergence::SnapshotText => {
            expected.text = "different".to_owned();
            valid_lease.clone()
        }
        Divergence::Transformed => {
            transformed = "different".to_owned();
            valid_lease.clone()
        }
        Divergence::LeaseSnapshot => {
            Arc::new(TransformLease::new(Uuid::new_v4(), receipt.request_id))
        }
        Divergence::LeaseRequest => {
            Arc::new(TransformLease::new(receipt.snapshot_id, Uuid::new_v4()))
        }
    };

    let result = state.restore(mutation_id, expected, transformed, Some(lease));

    assert!(matches!(result, Err(VerbalixError::StaleSelection)));
    assert_meta_eq(record_meta(&mut state, receipt.id), before);
    assert_eq!(instrumented.reads.get(), 0);
    assert!(valid_lease.try_claim_write());
}

#[test]
fn terminal_restore_rejects_every_divergent_replay_before_early_return_or_reconcile() {
    for outcome in [
        RestoreTerminalOutcome::Restored,
        RestoreTerminalOutcome::Indeterminate,
    ] {
        for divergence in [
            Divergence::MutationId,
            Divergence::SnapshotId,
            Divergence::SnapshotText,
            Divergence::Transformed,
            Divergence::LeaseSnapshot,
            Divergence::LeaseRequest,
        ] {
            assert_divergence_is_inert(outcome, divergence);
        }
    }
}

#[test]
fn identical_restored_replay_is_idempotent_without_read_or_claim() {
    let (mut state, expected, receipt, lease, instrumented) =
        terminal_state(RestoreTerminalOutcome::Restored);
    let before = record_meta(&mut state, receipt.id);

    let replay = state
        .restore(
            receipt.id,
            expected,
            "after".to_owned(),
            Some(lease.clone()),
        )
        .unwrap();

    assert_eq!(replay, receipt);
    assert_meta_eq(record_meta(&mut state, receipt.id), before);
    assert_eq!(instrumented.reads.get(), 0);
    assert!(lease.try_claim_write());
}

#[test]
fn identical_indeterminate_replay_reconciles_once_then_is_idempotent() {
    let (mut state, expected, receipt, lease, instrumented) =
        terminal_state(RestoreTerminalOutcome::Indeterminate);

    let first = state
        .restore(
            receipt.id,
            expected.clone(),
            "after".to_owned(),
            Some(lease.clone()),
        )
        .unwrap();
    let reconciled = record_meta(&mut state, receipt.id);
    let second = state
        .restore(
            receipt.id,
            expected,
            "after".to_owned(),
            Some(lease.clone()),
        )
        .unwrap();

    assert_eq!(first, receipt);
    assert_eq!(second, receipt);
    assert!(reconciled.status == MutationStatus::Restored);
    assert_meta_eq(record_meta(&mut state, receipt.id), reconciled);
    assert_eq!(instrumented.reads.get(), 1);
    assert!(lease.try_claim_write());
}
