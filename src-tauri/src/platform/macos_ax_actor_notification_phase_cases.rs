use super::notification_phase_tests::{
    assert_interruption_revokes_write, BarrierPoint, Interruption, MutationKind,
};

fn assert_case(kind: MutationKind, interruption: Interruption, barrier: BarrierPoint) {
    assert_interruption_revokes_write(kind, interruption, barrier);
}

#[test]
fn exact_external_selection_revokes_armed_replace_outside_actor_fifo() {
    assert_case(
        MutationKind::Replace,
        Interruption::ExactEvent,
        BarrierPoint::Armed,
    );
}

#[test]
fn exact_external_selection_revokes_armed_restore_outside_actor_fifo() {
    assert_case(
        MutationKind::Restore,
        Interruption::ExactEvent,
        BarrierPoint::Armed,
    );
}

#[test]
fn exact_external_selection_cancels_authorizing_replace_outside_actor_fifo() {
    assert_case(
        MutationKind::Replace,
        Interruption::ExactEvent,
        BarrierPoint::Authorizing,
    );
}

#[test]
fn exact_external_selection_cancels_authorizing_restore_outside_actor_fifo() {
    assert_case(
        MutationKind::Restore,
        Interruption::ExactEvent,
        BarrierPoint::Authorizing,
    );
}

#[test]
fn stale_epoch_cancels_authorizing_replace_before_setter() {
    assert_case(
        MutationKind::Replace,
        Interruption::StaleEpoch,
        BarrierPoint::Authorizing,
    );
}

#[test]
fn stale_epoch_cancels_authorizing_restore_before_setter() {
    assert_case(
        MutationKind::Restore,
        Interruption::StaleEpoch,
        BarrierPoint::Authorizing,
    );
}

#[test]
fn focus_after_epoch_check_cancels_replace_before_promotion() {
    assert_case(
        MutationKind::Replace,
        Interruption::FocusChanged,
        BarrierPoint::EpochValid,
    );
}

#[test]
fn focus_after_epoch_check_cancels_restore_before_promotion() {
    assert_case(
        MutationKind::Restore,
        Interruption::FocusChanged,
        BarrierPoint::EpochValid,
    );
}

#[test]
fn destroy_after_epoch_check_cancels_replace_before_promotion() {
    assert_case(
        MutationKind::Replace,
        Interruption::ElementDestroyed,
        BarrierPoint::EpochValid,
    );
}

#[test]
fn destroy_after_epoch_check_cancels_restore_before_promotion() {
    assert_case(
        MutationKind::Restore,
        Interruption::ElementDestroyed,
        BarrierPoint::EpochValid,
    );
}

#[test]
fn direct_signal_after_epoch_check_cancels_before_promotion() {
    assert_case(
        MutationKind::Replace,
        Interruption::DirectSignal,
        BarrierPoint::EpochValid,
    );
}
