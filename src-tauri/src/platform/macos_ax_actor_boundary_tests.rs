use super::*;
use crate::{
    application::{MutationStatus, TransformLease},
    domain::{
        Rect, SelectionElementIdentity, SelectionExtractionStrategy, SelectionSnapshot, TextRange,
        VerbalixError,
    },
    platform::{
        macos_mutation_ledger::{ReplaceTerminalOutcome, RestoreTerminalOutcome},
        macos_replace::WriteOutcome,
        macos_restore::RestoreWriteOutcome,
        macos_selection_revalidation::CurrentSelection,
    },
};
use std::{
    cell::Cell,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

const TERMINAL_TTL_MS: u64 = 600_000;

struct BoundaryEpochTarget {
    epoch: CausalEpoch,
    notifications: SelfNotificationSignal,
    writes: Cell<usize>,
    setters: Cell<usize>,
    observed_pending: Cell<bool>,
}

impl BoundaryEpochTarget {
    fn new(epoch: CausalEpoch, notifications: SelfNotificationSignal) -> Rc<Self> {
        Rc::new(Self {
            epoch,
            notifications,
            writes: Cell::new(0),
            setters: Cell::new(0),
            observed_pending: Cell::new(false),
        })
    }

    fn authorize(&self, authorization: &AxWriteAuthorization) -> bool {
        self.writes.set(self.writes.get() + 1);
        self.observed_pending.set(self.notifications.has_pending());
        self.epoch.bump();
        if authorization.begin_setter() {
            self.setters.set(self.setters.get() + 1);
            true
        } else {
            false
        }
    }
}

impl AxMutationTarget for BoundaryEpochTarget {
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

fn receipt_and_lease(selected: &SelectionSnapshot) -> (MutationReceipt, Arc<TransformLease>) {
    let request_id = Uuid::new_v4();
    (
        MutationReceipt {
            id: Uuid::new_v4(),
            snapshot_id: selected.id,
            request_id,
        },
        Arc::new(TransformLease::new(selected.id, request_id)),
    )
}

fn state_with_target() -> (
    ActorState,
    SelectionSnapshot,
    Rc<BoundaryEpochTarget>,
    CapturedTarget,
) {
    let epoch = CausalEpoch::default();
    let notifications = SelfNotificationSignal::default();
    let mut state = ActorState::new(epoch.clone(), notifications.clone());
    let selected = snapshot();
    let instrumented = BoundaryEpochTarget::new(epoch.clone(), notifications);
    let target = CapturedTarget {
        target: instrumented.clone(),
        epoch: epoch.current(),
        token: AxElementToken::new(selected.pid, "editor"),
    };
    state
        .targets
        .insert(selected.id, target.clone(), state.now());
    (state, selected, instrumented, target)
}

fn assert_rejected_at_final_boundary(
    state: &mut ActorState,
    id: Uuid,
    expected_status: MutationStatus,
    target: &BoundaryEpochTarget,
    lease: &TransformLease,
) {
    let projection = state.mutations.projection(id, state.now()).unwrap();
    assert!(projection.status == expected_status);
    assert_eq!(target.writes.get(), 1);
    assert_eq!(target.setters.get(), 0);
    assert!(target.observed_pending.get());
    assert!(!lease.try_claim_write());
    assert!(!state.self_notifications.has_pending());
}

#[test]
fn replace_rejects_epoch_bump_after_claim_and_arm_before_setter() {
    let (mut state, selected, instrumented, _) = state_with_target();
    let (receipt, lease) = receipt_and_lease(&selected);

    let result = state.replace(
        receipt.clone(),
        selected,
        "after".to_owned(),
        Some(lease.clone()),
    );

    assert!(matches!(result, Err(VerbalixError::LocalFailure)));
    assert_rejected_at_final_boundary(
        &mut state,
        receipt.id,
        MutationStatus::Rejected,
        &instrumented,
        &lease,
    );
}

#[test]
fn restore_rejects_epoch_bump_after_claim_and_arm_before_setter() {
    let (mut state, selected, instrumented, target) = state_with_target();
    let (receipt, lease) = receipt_and_lease(&selected);
    state
        .mutations
        .prepare(
            receipt.clone(),
            selected.clone(),
            "after".to_owned(),
            target,
            state.now(),
        )
        .unwrap();
    state
        .mutations
        .finish_replace(receipt.id, ReplaceTerminalOutcome::Confirmed, state.now())
        .unwrap();

    let result = state.restore(
        receipt.id,
        selected,
        "after".to_owned(),
        Some(lease.clone()),
    );

    assert!(matches!(result, Err(VerbalixError::LocalFailure)));
    assert_rejected_at_final_boundary(
        &mut state,
        receipt.id,
        MutationStatus::RestoreRejected,
        &instrumented,
        &lease,
    );
}

#[test]
fn restored_replay_is_idempotent_before_ttl_and_stale_at_exact_expiry() {
    let (mut state, selected, instrumented, target) = state_with_target();
    let (receipt, lease) = receipt_and_lease(&selected);
    state
        .mutations
        .prepare(
            receipt.clone(),
            selected.clone(),
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
        .finish_restore(receipt.id, RestoreTerminalOutcome::Restored, state.now())
        .unwrap();
    let terminal_at = state
        .mutations
        .get_mut(receipt.id, state.now())
        .unwrap()
        .terminal_at
        .unwrap();

    let replay = state.restore(
        receipt.id,
        selected.clone(),
        "after".to_owned(),
        Some(lease.clone()),
    );

    assert_eq!(replay.unwrap(), receipt);
    assert_eq!(instrumented.writes.get(), 0);
    assert!(lease.try_claim_write());

    state.started = Instant::now() - Duration::from_millis(terminal_at + TERMINAL_TTL_MS);
    let expired = state.restore(receipt.id, selected, "after".to_owned(), Some(lease));

    assert!(matches!(expired, Err(VerbalixError::StaleSelection)));
    assert!(state
        .mutations
        .projection(receipt.id, state.now())
        .is_none());
    assert_eq!(instrumented.writes.get(), 0);
}
