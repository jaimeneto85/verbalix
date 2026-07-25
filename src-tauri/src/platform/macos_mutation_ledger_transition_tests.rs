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

fn record_state(ledger: &MutationLedger<()>, id: Uuid) -> (MutationStatus, Option<u64>, bool) {
    let record = ledger.records.get(&id).unwrap();
    (
        record.projection.status,
        record.terminal_at,
        record.restore_attempted,
    )
}
