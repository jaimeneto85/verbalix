use super::{
    macos_ax::OwnedAxElement,
    macos_ax_actor_state::{ActorState, CapturedTarget},
    macos_ax_target,
    macos_element_token::AxElementToken,
    macos_mutation_ledger::{ReplaceTerminalOutcome, RestoreTerminalOutcome},
    macos_selection_revalidation,
    macos_self_notification_phase::SelfNotificationPhase,
};
use crate::{
    application::MutationStatus,
    domain::{SelectionExtractionStrategy, SelectionSnapshot},
};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct ExpectedSelfNotification {
    mutation_id: Uuid,
    target_snapshot_id: Uuid,
    target: AxElementToken,
    generation: u64,
    expected_text: String,
    expected_location: i64,
    expected_length: usize,
    strategy: SelectionExtractionStrategy,
    phase: SelfNotificationPhase,
}

#[derive(Clone, Default)]
pub(super) struct SelfNotificationSignal {
    pending: Arc<Mutex<Option<ExpectedSelfNotification>>>,
}

impl SelfNotificationSignal {
    pub(super) fn has_pending(&self) -> bool {
        self.pending.lock().is_ok_and(|pending| pending.is_some())
    }

    fn arm(&self, expected: ExpectedSelfNotification) {
        if let Ok(mut pending) = self.pending.lock() {
            if let Some(replaced) = pending.replace(expected) {
                replaced.phase.cancel();
            }
        }
    }

    pub(super) fn clear(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            if let Some(expected) = pending.take() {
                expected.phase.cancel();
            }
        }
    }

    pub(super) fn cancel_pending_before_setter(&self) -> bool {
        let Ok(mut pending) = self.pending.lock() else {
            return false;
        };
        let cancelled = pending
            .as_ref()
            .is_some_and(|expected| expected.phase.cancel_before_setter());
        if cancelled {
            pending.take();
        }
        cancelled
    }

    pub(super) fn take_exact(
        &self,
        target: AxElementToken,
        generation: u64,
    ) -> Option<ExpectedSelfNotification> {
        let expected = self.pending.lock().ok()?.take()?;
        let exact = expected.target == target && expected.generation == generation;
        if exact && expected.phase.claim_observation() {
            Some(expected)
        } else {
            expected.phase.cancel();
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    ) -> SelfNotificationPhase {
        let phase = SelfNotificationPhase::armed();
        let Some(target_token) = target.token.clone() else {
            self.self_notifications.clear();
            return phase;
        };
        self.self_notifications.arm(ExpectedSelfNotification {
            mutation_id,
            target_snapshot_id: snapshot.id,
            target: target_token,
            generation,
            expected_location: snapshot.range.location,
            expected_length: expected_text.encode_utf16().count(),
            expected_text,
            strategy: snapshot.extraction_strategy,
            phase: phase.clone(),
        });
        phase
    }

    pub(super) fn clear_self_notification(&mut self) {
        self.self_notifications.clear();
    }

    pub(super) fn observe_selection_change(
        &mut self,
        expected: ExpectedSelfNotification,
        target: AxElementToken,
        generation: u64,
    ) -> ObservedSelectionChange {
        let now = self.now();
        let Some((mutation_id, target_snapshot_id, status, current)) = self
            .mutations
            .get_mut(expected.mutation_id, now)
            .map(|record| {
                (
                    record.projection.receipt.id,
                    record.projection.target_snapshot_id,
                    record.projection.status,
                    record.target.target.read(expected.strategy),
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
            let _ =
                self.mutations
                    .reconcile_replace(id, ReplaceTerminalOutcome::Rejected, self.now());
        } else if status == MutationStatus::RestoreIndeterminate {
            let _ =
                self.mutations
                    .reconcile_restore(id, RestoreTerminalOutcome::Rejected, self.now());
        }
    }

    fn confirm_indeterminate_notification(&mut self, id: Uuid, status: MutationStatus) {
        if status == MutationStatus::Indeterminate {
            let _ =
                self.mutations
                    .reconcile_replace(id, ReplaceTerminalOutcome::Confirmed, self.now());
        } else if status == MutationStatus::RestoreIndeterminate {
            let _ =
                self.mutations
                    .reconcile_restore(id, RestoreTerminalOutcome::Restored, self.now());
        }
    }
}

pub(super) fn captured_target(
    element: Rc<OwnedAxElement>,
    epoch: u64,
    token: Option<AxElementToken>,
) -> CapturedTarget {
    CapturedTarget {
        target: macos_ax_target::native_target(element),
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
