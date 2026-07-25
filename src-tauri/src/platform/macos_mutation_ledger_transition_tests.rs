use super::*;
use crate::{
    application::{MutationReceipt, MutationStatus},
    domain::{Rect, SelectionSnapshot, TextRange},
};
use uuid::Uuid;

fn snapshot() -> SelectionSnapshot {
    SelectionSnapshot::new(
        42,
        "pid:42".to_owned(),
        "before".to_owned(),
        TextRange {
            location: 3,
            length: 6,
        },
        Rect {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        },
        true,
    )
}

#[test]
fn invalid_terminal_outcomes_preserve_status_ttl_and_restore_attempt() {
    let selected = snapshot();
    let receipt = MutationReceipt {
        id: Uuid::new_v4(),
        snapshot_id: selected.id,
        request_id: Uuid::new_v4(),
    };
    let mut ledger = MutationLedger::new(1);
    ledger
        .prepare(receipt.clone(), selected, "after".to_owned(), (), 0)
        .unwrap();
    let before = record_state(&ledger, receipt.id);
    assert!(ledger
        .reconcile_replace(receipt.id, ReplaceTerminalOutcome::Confirmed, 1)
        .is_err());
    assert!(record_state(&ledger, receipt.id) == before);
    ledger
        .finish_replace(receipt.id, ReplaceTerminalOutcome::Confirmed, 2)
        .unwrap();
    let confirmed = record_state(&ledger, receipt.id);
    ledger
        .finish_replace(receipt.id, ReplaceTerminalOutcome::Confirmed, 99)
        .unwrap();
    assert!(record_state(&ledger, receipt.id) == confirmed);
    ledger.begin_restore(receipt.id, 3).unwrap();
    let restoring = record_state(&ledger, receipt.id);
    assert!(ledger
        .finish_replace(receipt.id, ReplaceTerminalOutcome::Rejected, 4)
        .is_err());
    assert!(record_state(&ledger, receipt.id) == restoring);
    ledger
        .finish_restore(receipt.id, RestoreTerminalOutcome::Rejected, 5)
        .unwrap();
    let rejected = record_state(&ledger, receipt.id);
    ledger
        .finish_restore(receipt.id, RestoreTerminalOutcome::Rejected, 100)
        .unwrap();
    assert!(record_state(&ledger, receipt.id) == rejected);
}

#[test]
fn same_outcome_is_idempotent_only_for_the_api_phase_that_committed_it() {
    for outcome in [
        ReplaceTerminalOutcome::Confirmed,
        ReplaceTerminalOutcome::Rejected,
    ] {
        let (mut ledger, receipt) = prepared_ledger();
        ledger.finish_replace(receipt.id, outcome, 1).unwrap();
        let committed = record_state(&ledger, receipt.id);
        ledger.finish_replace(receipt.id, outcome, 99).unwrap();
        assert!(record_state(&ledger, receipt.id) == committed);
        assert!(ledger.reconcile_replace(receipt.id, outcome, 99).is_err());
        assert!(record_state(&ledger, receipt.id) == committed);
    }

    for outcome in [
        ReplaceTerminalOutcome::Confirmed,
        ReplaceTerminalOutcome::Rejected,
        ReplaceTerminalOutcome::Indeterminate,
    ] {
        let (mut ledger, receipt) = indeterminate_replace_ledger();
        ledger.reconcile_replace(receipt.id, outcome, 2).unwrap();
        let reconciled = record_state(&ledger, receipt.id);
        ledger.reconcile_replace(receipt.id, outcome, 99).unwrap();
        assert!(record_state(&ledger, receipt.id) == reconciled);
        assert!(ledger.finish_replace(receipt.id, outcome, 99).is_err());
        assert!(record_state(&ledger, receipt.id) == reconciled);
    }

    for outcome in [
        RestoreTerminalOutcome::Restored,
        RestoreTerminalOutcome::Rejected,
    ] {
        let (mut ledger, receipt) = restoring_ledger();
        ledger.finish_restore(receipt.id, outcome, 2).unwrap();
        let committed = record_state(&ledger, receipt.id);
        ledger.finish_restore(receipt.id, outcome, 99).unwrap();
        assert!(record_state(&ledger, receipt.id) == committed);
        assert!(ledger.reconcile_restore(receipt.id, outcome, 99).is_err());
        assert!(record_state(&ledger, receipt.id) == committed);
    }

    for outcome in [
        RestoreTerminalOutcome::Restored,
        RestoreTerminalOutcome::Rejected,
        RestoreTerminalOutcome::Indeterminate,
    ] {
        let (mut ledger, receipt) = indeterminate_restore_ledger();
        ledger.reconcile_restore(receipt.id, outcome, 4).unwrap();
        let reconciled = record_state(&ledger, receipt.id);
        ledger.reconcile_restore(receipt.id, outcome, 99).unwrap();
        assert!(record_state(&ledger, receipt.id) == reconciled);
        assert!(ledger.finish_restore(receipt.id, outcome, 99).is_err());
        assert!(record_state(&ledger, receipt.id) == reconciled);
    }
}

fn prepared_ledger() -> (MutationLedger<()>, MutationReceipt) {
    let selected = snapshot();
    let receipt = MutationReceipt {
        id: Uuid::new_v4(),
        snapshot_id: selected.id,
        request_id: Uuid::new_v4(),
    };
    let mut ledger = MutationLedger::new(1);
    ledger
        .prepare(receipt.clone(), selected, "after".to_owned(), (), 0)
        .unwrap();
    (ledger, receipt)
}

fn restoring_ledger() -> (MutationLedger<()>, MutationReceipt) {
    let (mut ledger, receipt) = prepared_ledger();
    ledger
        .finish_replace(receipt.id, ReplaceTerminalOutcome::Confirmed, 1)
        .unwrap();
    ledger.begin_restore(receipt.id, 2).unwrap();
    (ledger, receipt)
}

fn indeterminate_replace_ledger() -> (MutationLedger<()>, MutationReceipt) {
    let (mut ledger, receipt) = prepared_ledger();
    ledger
        .finish_replace(receipt.id, ReplaceTerminalOutcome::Indeterminate, 1)
        .unwrap();
    (ledger, receipt)
}

fn indeterminate_restore_ledger() -> (MutationLedger<()>, MutationReceipt) {
    let (mut ledger, receipt) = restoring_ledger();
    ledger
        .finish_restore(receipt.id, RestoreTerminalOutcome::Indeterminate, 3)
        .unwrap();
    (ledger, receipt)
}

fn record_state(
    ledger: &MutationLedger<()>,
    id: Uuid,
) -> (MutationStatus, Option<u64>, bool, Option<TerminalPhase>) {
    let record = ledger.records.get(&id).unwrap();
    (
        record.projection.status,
        record.terminal_at,
        record.restore_attempted,
        record.terminal_phase,
    )
}
