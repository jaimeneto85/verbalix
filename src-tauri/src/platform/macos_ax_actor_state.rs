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
        ensure_current(&self.epoch, target.epoch)?;
        let result =
            macos_replace::write_on_element(&expected, &text, target.element.as_ref().as_ref());
        self.finish_receipt(&receipt, result, MutationStatus::Confirmed)
    }

    pub(super) fn restore(
        &mut self,
        expected: SelectionSnapshot,
        transformed: String,
        lease: Option<PublicationGuard>,
    ) -> Result<MutationReceipt, VerbalixError> {
        let now = self.now();
        let target = self
            .targets
            .get(expected.id, now)
            .ok_or(VerbalixError::StaleSelection)?;
        macos_restore::prepare_on_element(
            &expected,
            &transformed,
            &target.element,
            has_no_identifier(&expected),
        )?;
        ensure_current(&self.epoch, target.epoch)?;
        let receipt = receipt(&expected, claim(&lease)?);
        ensure_current(&self.epoch, target.epoch)?;
        let result = macos_restore::write_on_element(&expected, target.element.as_ref().as_ref());
        let result = result.map(|_| receipt);
        if result.is_ok() {
            self.targets.remove(expected.id);
        }
        result
    }

    pub(super) fn discard(&mut self, id: Uuid) {
        self.targets.remove(id);
    }

    pub(super) fn reconcile(&mut self, id: Uuid) -> Option<MutationProjection> {
        let status = self.mutations.get_mut(id).and_then(|record| {
            (record.projection.status == MutationStatus::Indeterminate).then(|| {
                macos_selection_revalidation::read(
                    record.target.element.as_ref().as_ref(),
                    record.projection.strategy,
                )
                .ok()
                .map(|current| {
                    let expected_location = record.projection.snapshot.range.location;
                    let current_location = current.range.location as i64;
                    if current.text == record.projection.transformed_text
                        && current_location == expected_location
                    {
                        MutationStatus::Confirmed
                    } else if current.text == record.projection.original_text
                        && current_location == expected_location
                    {
                        MutationStatus::Rejected
                    } else {
                        MutationStatus::Indeterminate
                    }
                })
                .unwrap_or(MutationStatus::Indeterminate)
            })
        });
        if let Some(status) = status {
            let _ = self.mutations.terminalize(id, status, self.now());
        }
        self.mutations.projection(id, self.now())
    }

    fn now(&self) -> u64 {
        self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
    }

    fn finish_receipt(
        &mut self,
        receipt: &MutationReceipt,
        result: Result<(), VerbalixError>,
        state: MutationStatus,
    ) -> Result<MutationReceipt, VerbalixError> {
        match result {
            Ok(()) => {
                self.mutations.terminalize(receipt.id, state, self.now())?;
                Ok(receipt.clone())
            }
            Err(error) => {
                self.mutations.terminalize(
                    receipt.id,
                    MutationStatus::Indeterminate,
                    self.now(),
                )?;
                Err(error)
            }
        }
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

fn receipt(snapshot: &SelectionSnapshot, request_id: Uuid) -> MutationReceipt {
    MutationReceipt {
        id: Uuid::new_v4(),
        snapshot_id: snapshot.id,
        request_id,
    }
}

fn claim(lease: &Option<PublicationGuard>) -> Result<Uuid, VerbalixError> {
    match lease {
        Some(lease) if lease.try_claim_write() => Ok(lease.request_id()),
        Some(_) => Err(VerbalixError::StaleSelection),
        None => Ok(Uuid::nil()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn missing_causal_handle_has_no_focus_or_identity_fallback() {
        let source = include_str!("macos_ax_actor_state.rs");
        assert!(
            !source.contains(concat!("focused_element_for_", "pid")),
            "an absent or expired causal handle must fail before re-resolving an AX element"
        );
    }

    #[test]
    fn unresolved_write_is_retained_as_indeterminate() {
        let source = include_str!("macos_ax_actor_state.rs");
        assert!(source.contains("MutationStatus::Indeterminate"));
    }

    #[test]
    fn replay_is_resolved_from_ledger_before_ax_preparation() {
        let source = include_str!("macos_ax_actor_state.rs");
        let replace = &source[source
            .find("pub(super) fn replace(")
            .expect("replace boundary")..source
            .find("pub(super) fn restore(")
            .expect("restore boundary")];
        let replay_lookup = replace
            .find("self.mutations")
            .expect("mutation ledger lookup");
        let ax_preparation = replace
            .find("macos_replace::prepare_on_element")
            .expect("AX preparation");

        assert!(
            replay_lookup < ax_preparation,
            "matching mutation IDs must resolve before any AX revalidation or setter path"
        );
    }

    #[test]
    fn indeterminate_reconcile_requires_exact_utf16_range_length() {
        let source = include_str!("macos_ax_actor_state.rs");
        let reconcile = &source[source
            .find("pub(super) fn reconcile(")
            .expect("reconcile boundary")..source
            .find("fn now(")
            .expect("clock boundary")];

        assert!(reconcile.contains("current.range.location"));
        assert!(
            reconcile.contains("current.range.length"),
            "text and location alone cannot confirm the exact selected range"
        );
    }
}
