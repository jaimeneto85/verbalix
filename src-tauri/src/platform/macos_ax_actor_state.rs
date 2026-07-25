use super::{
    causal_epoch::CausalEpoch,
    causal_registry::CausalRegistry,
    macos_ax::{self, OwnedAxElement},
    macos_mutation_ledger::MutationLedger,
    macos_replace, macos_restore, macos_selection, macos_selection_revalidation,
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
struct CapturedTarget {
    element: Rc<OwnedAxElement>,
    epoch: u64,
}

pub(super) struct ActorState {
    started: Instant,
    epoch: CausalEpoch,
    targets: CausalRegistry<CapturedTarget>,
    mutations: MutationLedger<CapturedTarget>,
}

impl ActorState {
    pub(super) fn new(epoch: CausalEpoch) -> Self {
        Self {
            started: Instant::now(),
            epoch,
            targets: CausalRegistry::new(REGISTRY_CAPACITY, REGISTRY_TTL_MS),
            mutations: MutationLedger::new(REGISTRY_CAPACITY),
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
            self.targets.insert(
                snapshot.id,
                CapturedTarget {
                    element: Rc::new(element),
                    epoch: captured_epoch,
                },
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
        let outcome =
            macos_replace::write_on_element(&expected, &text, target.element.as_ref().as_ref());
        let status = match outcome {
            macos_replace::WriteOutcome::Confirmed => MutationStatus::Confirmed,
            macos_replace::WriteOutcome::Rejected => MutationStatus::Rejected,
            macos_replace::WriteOutcome::Indeterminate => MutationStatus::Indeterminate,
        };
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
        self.mutations
            .set_status(mutation_id, MutationStatus::RestorePrepared, self.now())?;
        if ensure_current(&self.epoch, boundary_epoch).is_err() {
            self.mutations
                .set_status(mutation_id, MutationStatus::RestoreRejected, self.now())?;
            return Err(VerbalixError::StaleSelection);
        }
        let request_id = match claim(&lease) {
            Ok(request_id) => request_id,
            Err(error) => {
                self.mutations.set_status(
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
            self.mutations
                .set_status(mutation_id, MutationStatus::RestoreRejected, self.now())?;
            return Err(VerbalixError::StaleSelection);
        }
        let result = macos_restore::write_on_element(&expected, target.element.as_ref().as_ref());
        let status = if result.is_ok() {
            MutationStatus::Restored
        } else {
            MutationStatus::RestoreIndeterminate
        };
        self.mutations.set_status(mutation_id, status, self.now())?;
        if result.is_ok() {
            self.targets.remove(expected.id);
        }
        result.map(|_| projection.receipt)
    }

    pub(super) fn discard(&mut self, id: Uuid) {
        self.targets.remove(id);
    }

    pub(super) fn reconcile(&mut self, id: Uuid) -> Option<MutationProjection> {
        let recovery = self.mutations.get_mut(id).and_then(|record| {
            matches!(
                record.projection.status,
                MutationStatus::Indeterminate | MutationStatus::RestoreIndeterminate
            )
            .then(|| {
                let restoring = record.projection.status == MutationStatus::RestoreIndeterminate;
                macos_selection_revalidation::read(
                    record.target.element.as_ref().as_ref(),
                    record.projection.strategy,
                )
                .ok()
                .map(|current| {
                    let expected_location = record.projection.snapshot.range.location;
                    let current_location = current.range.location as i64;
                    let status = if restoring
                        && current.text == record.projection.original_text
                        && current_location == expected_location
                        && current.range.length as i64 == record.projection.snapshot.range.length
                    {
                        MutationStatus::Restored
                    } else if current.text == record.projection.transformed_text
                        && current_location == expected_location
                        && current.range.length as usize
                            == record.projection.transformed_text.encode_utf16().count()
                    {
                        MutationStatus::Confirmed
                    } else if current.text == record.projection.original_text
                        && current_location == expected_location
                        && current.range.length as i64 == record.projection.snapshot.range.length
                    {
                        MutationStatus::Rejected
                    } else {
                        MutationStatus::Indeterminate
                    };
                    (restoring, status)
                })
                .unwrap_or((restoring, MutationStatus::Indeterminate))
            })
        });
        if let Some((restoring, status)) = recovery {
            if restoring {
                let _ = self.mutations.set_status(id, status, self.now());
            } else {
                let _ = self.mutations.terminalize(id, status, self.now());
            }
        }
        self.mutations.projection(id, self.now())
    }

    fn now(&self) -> u64 {
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
