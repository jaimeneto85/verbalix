use super::{
    causal_epoch::CausalEpoch,
    causal_registry::CausalRegistry,
    macos_ax,
    macos_ax_actor_observation::{self, SelfNotificationSignal},
    macos_ax_target::AxMutationTarget,
    macos_element_token::AxElementToken,
    macos_mutation_ledger::{MutationLedger, ReplaceTerminalOutcome, RestoreTerminalOutcome},
    macos_replace, macos_restore, macos_selection,
};
use crate::{
    application::{MutationProjection, MutationReceipt, MutationStatus, PublicationGuard},
    domain::{SelectionSnapshot, VerbalixError},
};
use std::{rc::Rc, time::Instant};
use uuid::Uuid;

const REGISTRY_CAPACITY: usize = 64;
const REGISTRY_TTL_MS: u64 = 600_000;

#[derive(Clone)]
pub(super) struct CapturedTarget {
    pub(super) target: Rc<dyn AxMutationTarget>,
    pub(super) epoch: u64,
    pub(super) token: Option<AxElementToken>,
}

pub(super) struct ActorState {
    pub(super) started: Instant,
    pub(super) epoch: CausalEpoch,
    pub(super) targets: CausalRegistry<CapturedTarget>,
    pub(super) mutations: MutationLedger<CapturedTarget>,
    pub(super) self_notifications: SelfNotificationSignal,
}

impl ActorState {
    pub(super) fn new(epoch: CausalEpoch, self_notifications: SelfNotificationSignal) -> Self {
        Self {
            started: Instant::now(),
            epoch,
            targets: CausalRegistry::new(REGISTRY_CAPACITY, REGISTRY_TTL_MS),
            mutations: MutationLedger::new(REGISTRY_CAPACITY),
            self_notifications,
        }
    }

    pub(super) fn capture(&mut self) -> Result<SelectionSnapshot, VerbalixError> {
        let captured_epoch = self.epoch.current();
        let element =
            macos_ax::focused_element().map_err(|_| VerbalixError::SelectionUnavailable)?;
        let snapshot = macos_selection::capture(&element)?;
        if !self.epoch.is_current(captured_epoch) {
            return Err(VerbalixError::StaleSelection);
        }
        if snapshot.writable {
            let token = snapshot
                .native_element_identifier()
                .and_then(|identifier| AxElementToken::new(snapshot.pid, identifier));
            self.targets.insert(
                snapshot.id,
                macos_ax_actor_observation::captured_target(
                    Rc::new(element),
                    captured_epoch,
                    token,
                ),
                self.now(),
            );
        }
        Ok(snapshot)
    }

    pub(super) fn replace(
        &mut self,
        receipt: MutationReceipt,
        expected: SelectionSnapshot,
        text: String,
        lease: Option<PublicationGuard>,
    ) -> Result<MutationReceipt, VerbalixError> {
        let now = self.now();
        if let Some(existing) = self.mutations.replay(&receipt, &expected, &text, now)? {
            return projection_result(existing);
        }
        let target = self
            .targets
            .get(expected.id, now)
            .cloned()
            .ok_or(VerbalixError::StaleSelection)?;
        target
            .target
            .prepare_replace(&expected, target.token.is_none())?;
        ensure_current(&self.epoch, target.epoch)?;
        self.mutations.prepare(
            receipt.clone(),
            expected.clone(),
            text.clone(),
            target.clone(),
            now,
        )?;
        let request_id = match claim(&lease) {
            Ok(request_id) => request_id,
            Err(error) => {
                self.mutations.finish_replace(
                    receipt.id,
                    ReplaceTerminalOutcome::Rejected,
                    self.now(),
                )?;
                return Err(error);
            }
        };
        if request_id != receipt.request_id {
            self.mutations.finish_replace(
                receipt.id,
                ReplaceTerminalOutcome::Rejected,
                self.now(),
            )?;
            return Err(VerbalixError::StaleSelection);
        }
        if ensure_current(&self.epoch, target.epoch).is_err() {
            self.mutations.finish_replace(
                receipt.id,
                ReplaceTerminalOutcome::Rejected,
                self.now(),
            )?;
            return Err(VerbalixError::StaleSelection);
        }
        self.arm_self_notification(receipt.id, &expected, &target, target.epoch, text.clone());
        let outcome = target.target.write_replace(&expected, &text);
        let terminal_outcome = match outcome {
            macos_replace::WriteOutcome::Confirmed => ReplaceTerminalOutcome::Confirmed,
            macos_replace::WriteOutcome::Rejected => ReplaceTerminalOutcome::Rejected,
            macos_replace::WriteOutcome::Indeterminate => ReplaceTerminalOutcome::Indeterminate,
        };
        if terminal_outcome == ReplaceTerminalOutcome::Rejected {
            self.clear_self_notification();
        }
        let projection = self
            .mutations
            .finish_replace(receipt.id, terminal_outcome, self.now())?;
        projection_result(projection)
    }

    pub(super) fn restore(
        &mut self,
        mutation_id: Uuid,
        expected: SelectionSnapshot,
        transformed: String,
        lease: Option<PublicationGuard>,
    ) -> Result<MutationReceipt, VerbalixError> {
        let (projection, target) = self
            .mutations
            .get_mut(mutation_id)
            .map(|record| (record.projection.clone(), record.target.clone()))
            .ok_or(VerbalixError::StaleSelection)?;
        if !restore_correlates(mutation_id, &projection, &expected, &transformed, &lease) {
            return Err(VerbalixError::StaleSelection);
        }
        if projection.status == MutationStatus::Restored {
            return Ok(projection.receipt);
        }
        if projection.status == MutationStatus::RestoreIndeterminate {
            return self
                .reconcile(mutation_id)
                .filter(|record| record.status == MutationStatus::Restored)
                .map(|record| record.receipt)
                .ok_or(VerbalixError::LocalFailure);
        }
        if projection.status != MutationStatus::Confirmed {
            return Err(VerbalixError::StaleSelection);
        }
        let boundary_epoch = target.epoch;
        ensure_current(&self.epoch, boundary_epoch)?;
        target
            .target
            .prepare_restore(&expected, &transformed, target.token.is_none())?;
        ensure_current(&self.epoch, boundary_epoch)?;
        self.mutations.begin_restore(mutation_id, self.now())?;
        if ensure_current(&self.epoch, boundary_epoch).is_err() {
            self.mutations.finish_restore(
                mutation_id,
                RestoreTerminalOutcome::Rejected,
                self.now(),
            )?;
            return Err(VerbalixError::StaleSelection);
        }
        let request_id = match claim(&lease) {
            Ok(request_id) => request_id,
            Err(error) => {
                self.mutations.finish_restore(
                    mutation_id,
                    RestoreTerminalOutcome::Rejected,
                    self.now(),
                )?;
                return Err(error);
            }
        };
        if request_id != projection.receipt.request_id
            || ensure_current(&self.epoch, boundary_epoch).is_err()
        {
            self.mutations.finish_restore(
                mutation_id,
                RestoreTerminalOutcome::Rejected,
                self.now(),
            )?;
            return Err(VerbalixError::StaleSelection);
        }
        self.arm_self_notification(
            mutation_id,
            &expected,
            &target,
            boundary_epoch,
            expected.text.clone(),
        );
        let outcome = target.target.write_restore(&expected);
        let terminal_outcome = match outcome {
            macos_restore::RestoreWriteOutcome::Confirmed => RestoreTerminalOutcome::Restored,
            macos_restore::RestoreWriteOutcome::Rejected => RestoreTerminalOutcome::Rejected,
            macos_restore::RestoreWriteOutcome::Indeterminate => {
                RestoreTerminalOutcome::Indeterminate
            }
        };
        self.mutations
            .finish_restore(mutation_id, terminal_outcome, self.now())?;
        if terminal_outcome == RestoreTerminalOutcome::Rejected {
            self.clear_self_notification();
        }
        if terminal_outcome == RestoreTerminalOutcome::Restored {
            self.targets.remove(expected.id);
            Ok(projection.receipt)
        } else {
            Err(VerbalixError::LocalFailure)
        }
    }

    pub(super) fn discard(&mut self, id: Uuid) {
        self.targets.remove(id);
    }

    pub(super) fn now(&self) -> u64 {
        self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
    }
}

fn projection_result(projection: MutationProjection) -> Result<MutationReceipt, VerbalixError> {
    match projection.status {
        MutationStatus::Confirmed => Ok(projection.receipt),
        MutationStatus::Prepared
        | MutationStatus::Rejected
        | MutationStatus::Indeterminate
        | MutationStatus::RestorePrepared
        | MutationStatus::Restored
        | MutationStatus::RestoreRejected
        | MutationStatus::RestoreIndeterminate => Err(VerbalixError::LocalFailure),
    }
}

fn ensure_current(epoch: &CausalEpoch, expected: u64) -> Result<(), VerbalixError> {
    epoch
        .is_current(expected)
        .then_some(())
        .ok_or(VerbalixError::StaleSelection)
}

fn restore_correlates(
    mutation_id: Uuid,
    projection: &MutationProjection,
    expected: &SelectionSnapshot,
    transformed: &str,
    lease: &Option<PublicationGuard>,
) -> bool {
    projection.receipt.id == mutation_id
        && projection.receipt.snapshot_id == expected.id
        && projection.target_snapshot_id == expected.id
        && projection.snapshot.same_target(expected)
        && projection.transformed_text == transformed
        && lease
            .as_ref()
            .map_or(projection.receipt.request_id.is_nil(), |lease| {
                lease.owns(
                    projection.receipt.snapshot_id,
                    projection.receipt.request_id,
                )
            })
}

fn claim(lease: &Option<PublicationGuard>) -> Result<Uuid, VerbalixError> {
    match lease {
        Some(lease) if lease.try_claim_write() => Ok(lease.request_id()),
        Some(_) => Err(VerbalixError::StaleSelection),
        None => Ok(Uuid::nil()),
    }
}

#[cfg(test)]
#[path = "macos_ax_actor_state_tests.rs"]
mod tests;
