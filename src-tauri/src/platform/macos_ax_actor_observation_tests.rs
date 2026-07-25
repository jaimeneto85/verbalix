use super::*;
use crate::{
    domain::SelectionExtractionStrategy,
    platform::{
        macos_ax::AxElementToken, macos_classic_range::CFRange,
        macos_selection_revalidation::CurrentSelection,
    },
};

#[test]
fn self_notification_requires_exact_mutation_target_generation_and_utf16_selection() {
    let mutation_id = Uuid::new_v4();
    let target_snapshot_id = Uuid::new_v4();
    let target = AxElementToken { pid: 42, hash: 7 };
    let expected = ExpectedSelfNotification {
        mutation_id,
        target_snapshot_id,
        target,
        generation: 11,
        expected_text: "depois 👩🏽‍💻".to_owned(),
        expected_location: 3,
        expected_length: "depois 👩🏽‍💻".encode_utf16().count(),
        strategy: SelectionExtractionStrategy::ValueRange,
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
        target,
        11,
        &current,
    ));
    for mismatch in [
        matches_expected_self_notification(
            &expected,
            Uuid::new_v4(),
            target_snapshot_id,
            target,
            11,
            &current,
        ),
        matches_expected_self_notification(
            &expected,
            mutation_id,
            Uuid::new_v4(),
            target,
            11,
            &current,
        ),
        matches_expected_self_notification(
            &expected,
            mutation_id,
            target_snapshot_id,
            AxElementToken { pid: 42, hash: 8 },
            11,
            &current,
        ),
        matches_expected_self_notification(
            &expected,
            mutation_id,
            target_snapshot_id,
            target,
            12,
            &current,
        ),
    ] {
        assert!(!mismatch);
    }
}

fn expectation(target: AxElementToken, generation: u64) -> ExpectedSelfNotification {
    ExpectedSelfNotification {
        mutation_id: Uuid::new_v4(),
        target_snapshot_id: Uuid::new_v4(),
        target,
        generation,
        expected_text: "after".to_owned(),
        expected_location: 0,
        expected_length: 5,
        strategy: SelectionExtractionStrategy::SelectedText,
    }
}

#[test]
fn self_notification_expectation_is_consumed_once_even_on_mismatch() {
    let target = AxElementToken { pid: 42, hash: 7 };
    let mut exact = Some(expectation(target, 11));
    assert!(take_expected_self_notification(&mut exact, target, 11).is_some());
    assert!(take_expected_self_notification(&mut exact, target, 11).is_none());

    let mut mismatch = Some(expectation(target, 11));
    assert!(take_expected_self_notification(
        &mut mismatch,
        AxElementToken { pid: 42, hash: 8 },
        11,
    )
    .is_none());
    assert!(mismatch.is_none());
}
