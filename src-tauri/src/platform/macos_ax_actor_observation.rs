use super::{
    macos_ax::{AxElementToken, OwnedAxElement},
    macos_ax_actor_state::{ActorState, CapturedTarget},
    macos_selection_revalidation,
};
use crate::{
    application::MutationStatus,
    domain::{SelectionExtractionStrategy, SelectionSnapshot},
};
use std::rc::Rc;
use uuid::Uuid;

pub(super) struct ExpectedSelfNotification {
    mutation_id: Uuid,
    target_snapshot_id: Uuid,
    target: AxElementToken,
    generation: u64,
    expected_text: String,
    expected_location: i64,
    expected_length: usize,
    strategy: SelectionExtractionStrategy,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ObservedSelectionChange {
    SelfGenerated,
    External,
}

impl ActorState {
    pub(super) fn arm_self_notification(
        &mut self,
        mutation_id: Uuid,
        snapshot: &SelectionSnapshot,
        target: &CapturedTarget,
        generation: u64,
        expected_text: String,
    ) {
        self.expected_self_notification = Some(ExpectedSelfNotification {
            mutation_id,
            target_snapshot_id: snapshot.id,
            target: target.token,
            generation,
            expected_location: snapshot.range.location,
            expected_length: expected_text.encode_utf16().count(),
            expected_text,
            strategy: snapshot.extraction_strategy,
        });
    }

    pub(super) fn clear_self_notification(&mut self) {
        self.expected_self_notification = None;
    }

    pub(super) fn observe_selection_change(
        &mut self,
        target: AxElementToken,
        generation: u64,
    ) -> ObservedSelectionChange {
        let Some(expected) = take_expected_self_notification(
            &mut self.expected_self_notification,
            target,
            generation,
        ) else {
            return ObservedSelectionChange::External;
        };
        let Some((mutation_id, target_snapshot_id, status, current)) =
            self.mutations.get_mut(expected.mutation_id).map(|record| {
                (
                    record.projection.receipt.id,
                    record.projection.target_snapshot_id,
                    record.projection.status,
                    macos_selection_revalidation::read(
                        record.target.element.as_ref().as_ref(),
                        expected.strategy,
                    ),
                )
            })
        else {
            return ObservedSelectionChange::External;
        };
        if target_snapshot_id != expected.target_snapshot_id {
            return ObservedSelectionChange::External;
        }
        let Ok(current) = current else {
            self.reject_unverifiable_notification(expected.mutation_id, status);
            return ObservedSelectionChange::External;
        };
        if !matches_expected_self_notification(
            &expected,
            mutation_id,
            target_snapshot_id,
            target,
            generation,
            &current,
        ) {
            return ObservedSelectionChange::External;
        }
        self.confirm_indeterminate_notification(expected.mutation_id, status);
        ObservedSelectionChange::SelfGenerated
    }

    fn reject_unverifiable_notification(&mut self, id: Uuid, status: MutationStatus) {
        if status == MutationStatus::Indeterminate {
            let _ = self
                .mutations
                .terminalize(id, MutationStatus::Rejected, self.now());
        } else if status == MutationStatus::RestoreIndeterminate {
            let _ =
                self.mutations
                    .reconcile_restore(id, MutationStatus::RestoreRejected, self.now());
        }
    }

    fn confirm_indeterminate_notification(&mut self, id: Uuid, status: MutationStatus) {
        if status == MutationStatus::Indeterminate {
            let _ = self
                .mutations
                .terminalize(id, MutationStatus::Confirmed, self.now());
        } else if status == MutationStatus::RestoreIndeterminate {
            let _ = self
                .mutations
                .reconcile_restore(id, MutationStatus::Restored, self.now());
        }
    }
}

fn take_expected_self_notification(
    pending: &mut Option<ExpectedSelfNotification>,
    target: AxElementToken,
    generation: u64,
) -> Option<ExpectedSelfNotification> {
    pending
        .take()
        .filter(|expected| expected.target == target && expected.generation == generation)
}

pub(super) fn captured_target(
    element: Rc<OwnedAxElement>,
    epoch: u64,
    token: AxElementToken,
) -> CapturedTarget {
    CapturedTarget {
        element,
        epoch,
        token,
    }
}

fn matches_expected_self_notification(
    expected: &ExpectedSelfNotification,
    mutation_id: Uuid,
    target_snapshot_id: Uuid,
    target: AxElementToken,
    generation: u64,
    current: &macos_selection_revalidation::CurrentSelection,
) -> bool {
    expected.mutation_id == mutation_id
        && expected.target_snapshot_id == target_snapshot_id
        && expected.target == target
        && expected.generation == generation
        && expected.expected_text == current.text
        && expected.expected_location == current.range.location as i64
        && expected.expected_length == current.range.length as usize
        && expected.strategy == current.strategy
}

#[cfg(test)]
#[path = "macos_ax_actor_observation_tests.rs"]
mod tests;
