use super::*;
use crate::domain::{Rect, SelectionSnapshot, TextRange};
use crate::{
    application::MutationReceipt,
    domain::SelectionExtractionStrategy,
    platform::{
        macos_classic_range::CFRange, macos_element_token::AxElementToken,
        macos_mutation_ledger::MutationLedger, macos_selection_revalidation::CurrentSelection,
    },
};

fn token(identifier: &str) -> AxElementToken {
    AxElementToken::new(42, identifier).unwrap()
}

#[test]
fn self_notification_requires_exact_mutation_target_generation_and_utf16_selection() {
    let mutation_id = Uuid::new_v4();
    let target_snapshot_id = Uuid::new_v4();
    let target = token("editor");
    let expected = ExpectedSelfNotification {
        mutation_id,
        target_snapshot_id,
        target: target.clone(),
        generation: 11,
        expected_text: "depois 👩🏽‍💻".to_owned(),
        expected_location: 3,
        expected_length: "depois 👩🏽‍💻".encode_utf16().count(),
        strategy: SelectionExtractionStrategy::ValueRange,
        phase: SelfNotificationPhase::armed(),
    };
    let current = CurrentSelection {
        text: expected.expected_text.clone(),
        range: CFRange {
            location: 3,
            length: expected.expected_length as isize,
        },
        strategy: SelectionExtractionStrategy::ValueRange,
    };

    assert!(matches_expected_self_notification(
        &expected,
        mutation_id,
        target_snapshot_id,
        target.clone(),
        11,
        &current,
    ));
    for mismatch in [
        matches_expected_self_notification(
            &expected,
            Uuid::new_v4(),
            target_snapshot_id,
            target.clone(),
            11,
            &current,
        ),
        matches_expected_self_notification(
            &expected,
            mutation_id,
            Uuid::new_v4(),
            target.clone(),
            11,
            &current,
        ),
        matches_expected_self_notification(
            &expected,
            mutation_id,
            target_snapshot_id,
            token("another-editor"),
            11,
            &current,
        ),
        matches_expected_self_notification(
            &expected,
            mutation_id,
            target_snapshot_id,
            target.clone(),
            12,
            &current,
        ),
    ] {
        assert!(!mismatch);
    }
}

fn expectation(target: AxElementToken, generation: u64) -> ExpectedSelfNotification {
    let phase = SelfNotificationPhase::armed();
    assert!(phase.begin_setter());
    ExpectedSelfNotification {
        mutation_id: Uuid::new_v4(),
        target_snapshot_id: Uuid::new_v4(),
        target,
        generation,
        expected_text: "after".to_owned(),
        expected_location: 0,
        expected_length: 5,
        strategy: SelectionExtractionStrategy::SelectedText,
        phase,
    }
}

#[test]
fn self_notification_expectation_is_consumed_once_even_on_mismatch() {
    let target = token("editor");
    let exact = SelfNotificationSignal::default();
    exact.arm(expectation(target.clone(), 11));
    assert!(exact.take_exact(target.clone(), 11).is_some());
    assert!(exact.take_exact(target.clone(), 11).is_none());

    let mismatch = SelfNotificationSignal::default();
    mismatch.arm(expectation(target.clone(), 11));
    assert!(mismatch.take_exact(token("another-editor"), 11,).is_none());
    assert!(mismatch.take_exact(target, 11).is_none());
}

#[test]
fn exact_expectation_matches_actor_ledger_and_current_selection_once() {
    let selected = SelectionSnapshot::new(
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
    .with_extraction_strategy(SelectionExtractionStrategy::ValueRange);
    let receipt = MutationReceipt {
        id: Uuid::new_v4(),
        snapshot_id: selected.id,
        request_id: Uuid::new_v4(),
    };
    let target = token("editor");
    let generation = 11;
    let transformed = "after 👩🏽‍💻";
    let mut ledger = MutationLedger::new(1);
    ledger
        .prepare(
            receipt.clone(),
            selected.clone(),
            transformed.to_owned(),
            (),
            0,
        )
        .unwrap();
    ledger
        .finish_replace(
            receipt.id,
            crate::platform::macos_mutation_ledger::ReplaceTerminalOutcome::Confirmed,
            1,
        )
        .unwrap();
    let projection = ledger.projection(receipt.id, 1).unwrap();
    let signal = SelfNotificationSignal::default();
    let phase = SelfNotificationPhase::armed();
    assert!(phase.begin_setter());
    signal.arm(ExpectedSelfNotification {
        mutation_id: projection.receipt.id,
        target_snapshot_id: projection.target_snapshot_id,
        target: target.clone(),
        generation,
        expected_text: projection.transformed_text.clone(),
        expected_location: projection.snapshot.range.location,
        expected_length: projection.transformed_text.encode_utf16().count(),
        strategy: projection.strategy,
        phase,
    });
    let current = CurrentSelection {
        text: projection.transformed_text.clone(),
        range: CFRange {
            location: projection.snapshot.range.location as isize,
            length: projection.transformed_text.encode_utf16().count() as isize,
        },
        strategy: projection.strategy,
    };

    let expected = signal.take_exact(target.clone(), generation).unwrap();
    assert!(matches_expected_self_notification(
        &expected,
        projection.receipt.id,
        projection.target_snapshot_id,
        target.clone(),
        generation,
        &current,
    ));
    assert!(signal.take_exact(target.clone(), generation).is_none());

    let changed = CurrentSelection {
        text: "external".to_owned(),
        ..current
    };
    assert!(!matches_expected_self_notification(
        &expected,
        projection.receipt.id,
        projection.target_snapshot_id,
        target.clone(),
        generation,
        &changed,
    ));
}
