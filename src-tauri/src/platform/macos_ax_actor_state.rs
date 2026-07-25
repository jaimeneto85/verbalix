use super::{
    causal_registry::CausalRegistry,
    macos_ax::{self, OwnedAxElement},
    macos_replace, macos_restore, macos_selection,
};
use crate::{
    application::{MutationReceipt, PublicationGuard},
    domain::{SelectionSnapshot, VerbalixError},
};
use std::time::Instant;
use uuid::Uuid;

const REGISTRY_CAPACITY: usize = 64;
const REGISTRY_TTL_MS: u64 = 600_000;

enum ReceiptState {
    Intent,
    Applied,
    Restored,
}

struct StoredReceipt {
    receipt: MutationReceipt,
    state: ReceiptState,
}

pub(super) struct ActorState {
    started: Instant,
    targets: CausalRegistry<OwnedAxElement>,
    receipts: CausalRegistry<StoredReceipt>,
}

impl ActorState {
    pub(super) fn new() -> Self {
        Self {
            started: Instant::now(),
            targets: CausalRegistry::new(REGISTRY_CAPACITY, REGISTRY_TTL_MS),
            receipts: CausalRegistry::new(REGISTRY_CAPACITY, REGISTRY_TTL_MS),
        }
    }

    pub(super) fn capture(&mut self) -> Result<SelectionSnapshot, VerbalixError> {
        let element =
            macos_ax::focused_element().map_err(|_| VerbalixError::SelectionUnavailable)?;
        let snapshot = macos_selection::capture(&element)?;
        if snapshot.writable {
            self.targets.insert(snapshot.id, element, self.now());
        }
        Ok(snapshot)
    }

    pub(super) fn replace(
        &mut self,
        expected: SelectionSnapshot,
        text: String,
        lease: Option<PublicationGuard>,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.ensure_target(&expected)?;
        let causal = has_no_identifier(&expected);
        let now = self.now();
        let target = self
            .targets
            .get(expected.id, now)
            .ok_or(VerbalixError::StaleSelection)?;
        macos_replace::prepare_on_element(&expected, target, causal)?;
        let receipt = receipt(&expected, claim(&lease)?);
        self.receipts.insert(
            receipt.id,
            StoredReceipt {
                receipt: receipt.clone(),
                state: ReceiptState::Intent,
            },
            now,
        );
        let result = macos_replace::write_on_element(&expected, &text, target.as_ref());
        self.finish_receipt(&receipt, result, ReceiptState::Applied)
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
            target,
            has_no_identifier(&expected),
        )?;
        let receipt = receipt(&expected, claim(&lease)?);
        self.receipts.insert(
            receipt.id,
            StoredReceipt {
                receipt: receipt.clone(),
                state: ReceiptState::Intent,
            },
            now,
        );
        let result = macos_restore::write_on_element(&expected, target.as_ref());
        let result = self.finish_receipt(&receipt, result, ReceiptState::Restored);
        if result.is_ok() {
            self.targets.remove(expected.id);
        }
        result
    }

    pub(super) fn discard(&mut self, id: Uuid) {
        self.targets.remove(id);
    }

    fn now(&self) -> u64 {
        self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
    }

    fn ensure_target(&mut self, expected: &SelectionSnapshot) -> Result<(), VerbalixError> {
        let now = self.now();
        if self.targets.get(expected.id, now).is_some() {
            return Ok(());
        }
        if has_no_identifier(expected) {
            return Err(VerbalixError::StaleSelection);
        }
        let element = macos_ax::focused_element_for_pid(expected.pid)
            .map_err(|_| VerbalixError::StaleSelection)?;
        self.targets.insert(expected.id, element, now);
        Ok(())
    }

    fn finish_receipt(
        &mut self,
        receipt: &MutationReceipt,
        result: Result<(), VerbalixError>,
        state: ReceiptState,
    ) -> Result<MutationReceipt, VerbalixError> {
        match result {
            Ok(()) => {
                let stored = self
                    .receipts
                    .get_mut(receipt.id, self.now())
                    .ok_or(VerbalixError::LocalFailure)?;
                stored.state = state;
                Ok(stored.receipt.clone())
            }
            Err(error) => {
                self.receipts.remove(receipt.id);
                Err(error)
            }
        }
    }
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
