use super::*;
use crate::domain::{Rect, TextRange};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn snapshot() -> SelectionSnapshot {
    SelectionSnapshot::new(
        7,
        "pid:7".to_owned(),
        "before".to_owned(),
        TextRange {
            location: 0,
            length: 6,
        },
        Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        true,
    )
}

fn receipt(selected: &SelectionSnapshot) -> MutationReceipt {
    MutationReceipt {
        id: Uuid::new_v4(),
        snapshot_id: selected.id,
        request_id: Uuid::new_v4(),
    }
}

#[test]
fn prepared_survives_terminal_ttl_and_replay_is_idempotent() {
    let selected = snapshot();
    let receipt = receipt(&selected);
    let mut ledger = MutationLedger::new(1);
    ledger
        .prepare(receipt.clone(), selected.clone(), "after".to_owned(), (), 0)
        .unwrap();
    assert!(ledger.projection(receipt.id, TERMINAL_TTL_MS).is_some());
    assert!(
        ledger
            .prepare(receipt, selected, "after".to_owned(), (), TERMINAL_TTL_MS)
            .unwrap()
            .status
            == MutationStatus::Prepared
    );
}

#[test]
fn prepared_record_contains_complete_recovery_projection_before_setter() {
    let selected = snapshot();
    let receipt = receipt(&selected);
    let mut ledger = MutationLedger::new(1);
    let projection = ledger
        .prepare(
            receipt.clone(),
            selected.clone(),
            "after".to_owned(),
            "exact-target",
            9,
        )
        .unwrap();

    assert_eq!(projection.receipt, receipt);
    assert!(projection.snapshot.same_target(&selected));
    assert_eq!(projection.original_text, "before");
    assert_eq!(projection.transformed_text, "after");
    assert_eq!(projection.strategy, selected.extraction_strategy);
    assert_eq!(projection.target_snapshot_id, selected.id);
    assert!(projection.status == MutationStatus::Prepared);
    assert_eq!(ledger.get_mut(receipt.id).unwrap().target, "exact-target");
}

#[test]
fn prepared_is_pinned_and_capacity_rejects_before_setter() {
    let first_snapshot = snapshot();
    let first = receipt(&first_snapshot);
    let mut second_snapshot = snapshot();
    second_snapshot.id = Uuid::new_v4();
    let second = receipt(&second_snapshot);
    let setters = Arc::new(AtomicUsize::new(0));
    let mut ledger = MutationLedger::new(1);
    ledger
        .prepare(first, first_snapshot, "after".to_owned(), (), 0)
        .unwrap();

    let result = ledger.prepare(
        second,
        second_snapshot,
        "another".to_owned(),
        (),
        TERMINAL_TTL_MS + 1,
    );
    if result.is_ok() {
        setters.fetch_add(1, Ordering::SeqCst);
    }

    assert!(matches!(result, Err(VerbalixError::LocalFailure)));
    assert_eq!(setters.load(Ordering::SeqCst), 0);
}

#[test]
fn confirmed_response_loss_reconciles_same_projection_repeatedly() {
    let selected = snapshot();
    let receipt = receipt(&selected);
    let mut ledger = MutationLedger::new(1);
    ledger
        .prepare(receipt.clone(), selected, "after".to_owned(), (), 0)
        .unwrap();
    let confirmed = ledger
        .terminalize(receipt.id, MutationStatus::Confirmed, 1)
        .unwrap();

    for now in [1, 2, TERMINAL_TTL_MS] {
        let recovered = ledger.projection(receipt.id, now).unwrap();
        assert_eq!(recovered.receipt, confirmed.receipt);
        assert!(recovered.status == MutationStatus::Confirmed);
        assert_eq!(recovered.transformed_text, "after");
    }
}

#[test]
fn matching_replay_returns_terminal_outcome_and_divergent_replay_is_closed() {
    let selected = snapshot();
    let receipt = receipt(&selected);
    let mut ledger = MutationLedger::new(2);
    ledger
        .prepare(receipt.clone(), selected.clone(), "after".to_owned(), (), 0)
        .unwrap();
    ledger
        .terminalize(receipt.id, MutationStatus::Confirmed, 1)
        .unwrap();

    let replay = ledger
        .prepare(receipt.clone(), selected.clone(), "after".to_owned(), (), 2)
        .unwrap();
    assert!(replay.status == MutationStatus::Confirmed);
    assert!(matches!(
        ledger.prepare(receipt, selected, "divergent".to_owned(), (), 2),
        Err(VerbalixError::StaleSelection)
    ));
}

#[test]
fn restore_indeterminate_reconciled_as_confirmed_expires_after_terminal_ttl() {
    let selected = snapshot();
    let receipt = receipt(&selected);
    let mut ledger = MutationLedger::new(1);
    ledger
        .prepare(receipt.clone(), selected, "after".to_owned(), (), 0)
        .unwrap();
    ledger
        .terminalize(receipt.id, MutationStatus::Confirmed, 1)
        .unwrap();
    ledger
        .set_status(receipt.id, MutationStatus::RestoreIndeterminate, 2)
        .unwrap();
    ledger
        .set_status(receipt.id, MutationStatus::Confirmed, 3)
        .unwrap();

    assert!(ledger
        .projection(receipt.id, 3 + TERMINAL_TTL_MS - 1)
        .is_some());
    assert!(ledger
        .projection(receipt.id, 3 + TERMINAL_TTL_MS)
        .is_none());
}

#[test]
fn mutation_record_source_stays_actor_private_and_non_serializable() {
    let source = include_str!("macos_mutation_ledger.rs");
    let record = &source[source
        .find("pub(super) struct ActorMutationRecord")
        .expect("actor record")
        ..source
            .find("pub(super) struct MutationLedger")
            .expect("ledger")];
    assert!(!record.contains(concat!("derive(", "Debug")));
    assert!(!record.contains(concat!("Serialize", ")")));
    assert!(!source.contains(concat!("diagnostics", "::")));
    assert!(!source.contains(concat!("print", "ln!")));
}
