use super::{
    causal_epoch::CausalEpoch,
    causal_registry::CausalRegistry,
    macos_ax::{self, AxElementToken, OwnedAxElement},
    macos_ax_actor_observation::{self, ExpectedSelfNotification},
    macos_mutation_ledger::MutationLedger,
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
    pub(super) element: Rc<OwnedAxElement>,
    pub(super) epoch: u64,
    pub(super) token: AxElementToken,
}

pub(super) struct ActorState {
    pub(super) started: Instant,
    pub(super) epoch: CausalEpoch,
    pub(super) targets: CausalRegistry<CapturedTarget>,
    pub(super) mutations: MutationLedger<CapturedTarget>,
    pub(super) expected_self_notification: Option<ExpectedSelfNotification>,
}

impl ActorState {
    pub(super) fn new(epoch: CausalEpoch) -> Self {
        Self {
            started: Instant::now(),
            epoch,
            targets: CausalRegistry::new(REGISTRY_CAPACITY, REGISTRY_TTL_MS),
            mutations: MutationLedger::new(REGISTRY_CAPACITY),
            expected_self_notification: None,
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
            let token = macos_ax::element_token(element.as_ref())
                .map_err(|_| VerbalixError::SelectionUnavailable)?;
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
        let causal = has_no_identifier(&expected);
        let now = self.now();
        if let Some(existing) = self.mutations.replay(&receipt, &expected, &text, now)? {
            return projection_result(existing);
        }
        let target = self
            .targets
            .get(expected.id, now)
            .cloned()
            .ok_or(VerbalixError::StaleSelection)?;
        macos_replace::prepare_on_element(&expected, &target.element, causal)?;
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
                self.mutations
                    .terminalize(receipt.id, MutationStatus::Rejected, self.now())?;
                return Err(error);
            }
        };
        if request_id != receipt.request_id {
            self.mutations
                .terminalize(receipt.id, MutationStatus::Rejected, self.now())?;
            return Err(VerbalixError::StaleSelection);
        }
        if ensure_current(&self.epoch, target.epoch).is_err() {
            self.mutations
                .terminalize(receipt.id, MutationStatus::Rejected, self.now())?;
            return Err(VerbalixError::StaleSelection);
        }
        self.arm_self_notification(receipt.id, &expected, &target, target.epoch, text.clone());
        let outcome =
            macos_replace::write_on_element(&expected, &text, target.element.as_ref().as_ref());
        let status = match outcome {
            macos_replace::WriteOutcome::Confirmed => MutationStatus::Confirmed,
            macos_replace::WriteOutcome::Rejected => MutationStatus::Rejected,
            macos_replace::WriteOutcome::Indeterminate => MutationStatus::Indeterminate,
        };
        if status == MutationStatus::Rejected {
            self.clear_self_notification();
        }
        let projection = self.mutations.terminalize(receipt.id, status, self.now())?;
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
        if projection.status != MutationStatus::Confirmed
            || !projection.snapshot.same_target(&expected)
            || projection.transformed_text != transformed
        {
            return Err(VerbalixError::StaleSelection);
        }
        let boundary_epoch = self.epoch.current();
        macos_restore::prepare_on_element(
            &expected,
            &transformed,
            &target.element,
            has_no_identifier(&expected),
        )?;
        self.mutations.begin_restore(mutation_id, self.now())?;
        if ensure_current(&self.epoch, boundary_epoch).is_err() {
            self.mutations.finish_restore(
                mutation_id,
                MutationStatus::RestoreRejected,
                self.now(),
            )?;
            return Err(VerbalixError::StaleSelection);
        }
        let request_id = match claim(&lease) {
            Ok(request_id) => request_id,
            Err(error) => {
                self.mutations.finish_restore(
                    mutation_id,
                    MutationStatus::RestoreRejected,
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
                MutationStatus::RestoreRejected,
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
        let outcome = macos_restore::write_on_element(&expected, target.element.as_ref().as_ref());
        let status = match outcome {
            macos_restore::RestoreWriteOutcome::Confirmed => MutationStatus::Restored,
            macos_restore::RestoreWriteOutcome::Rejected => MutationStatus::RestoreRejected,
            macos_restore::RestoreWriteOutcome::Indeterminate => {
                MutationStatus::RestoreIndeterminate
            }
        };
        self.mutations
            .finish_restore(mutation_id, status, self.now())?;
        if status == MutationStatus::RestoreRejected {
            self.clear_self_notification();
        }
        if status == MutationStatus::Restored {
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

fn has_no_identifier(snapshot: &SelectionSnapshot) -> bool {
    snapshot
        .element_identity
        .as_ref()
        .and_then(|identity| identity.strong_identifier())
        .is_none()
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
