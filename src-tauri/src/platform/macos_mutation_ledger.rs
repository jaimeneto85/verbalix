use crate::{
    application::{MutationProjection, MutationReceipt, MutationStatus},
    domain::{SelectionSnapshot, VerbalixError},
};
use std::collections::HashMap;
use uuid::Uuid;

const TERMINAL_TTL_MS: u64 = 600_000;

pub(super) struct ActorMutationRecord<T> {
    pub(super) projection: MutationProjection,
    pub(super) target: T,
    terminal_at: Option<u64>,
}

pub(super) struct MutationLedger<T> {
    records: HashMap<Uuid, ActorMutationRecord<T>>,
    capacity: usize,
}

impl<T> MutationLedger<T> {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            records: HashMap::new(),
            capacity: capacity.max(1),
        }
    }

    pub(super) fn prepare(
        &mut self,
        receipt: MutationReceipt,
        snapshot: SelectionSnapshot,
        transformed_text: String,
        target: T,
        now_ms: u64,
    ) -> Result<MutationProjection, VerbalixError> {
        self.prune(now_ms);
        if let Some(existing) = self.records.get(&receipt.id) {
            return matching(existing, &receipt, &snapshot, &transformed_text)
                .then(|| existing.projection.clone())
                .ok_or(VerbalixError::StaleSelection);
        }
        if self.records.len() >= self.capacity {
            return Err(VerbalixError::LocalFailure);
        }
        let projection = MutationProjection {
            receipt: receipt.clone(),
            original_text: snapshot.text.clone(),
            transformed_text,
            strategy: snapshot.extraction_strategy,
            target_snapshot_id: snapshot.id,
            snapshot,
            status: MutationStatus::Prepared,
        };
        self.records.insert(
            receipt.id,
            ActorMutationRecord {
                projection: projection.clone(),
                target,
                terminal_at: None,
            },
        );
        Ok(projection)
    }

    pub(super) fn replay(
        &mut self,
        receipt: &MutationReceipt,
        snapshot: &SelectionSnapshot,
        transformed_text: &str,
        now_ms: u64,
    ) -> Result<Option<MutationProjection>, VerbalixError> {
        self.prune(now_ms);
        self.records
            .get(&receipt.id)
            .map(|record| {
                matching(record, receipt, snapshot, transformed_text)
                    .then(|| record.projection.clone())
                    .ok_or(VerbalixError::StaleSelection)
            })
            .transpose()
    }

    pub(super) fn get_mut(&mut self, id: Uuid) -> Option<&mut ActorMutationRecord<T>> {
        self.records.get_mut(&id)
    }

    pub(super) fn terminalize(
        &mut self,
        id: Uuid,
        status: MutationStatus,
        now_ms: u64,
    ) -> Result<MutationProjection, VerbalixError> {
        let record = self
            .records
            .get_mut(&id)
            .ok_or(VerbalixError::LocalFailure)?;
        if record.projection.status == MutationStatus::Prepared
            || record.projection.status == MutationStatus::Indeterminate
        {
            record.projection.status = status;
            record.terminal_at =
                matches!(status, MutationStatus::Confirmed | MutationStatus::Rejected)
                    .then_some(now_ms);
        }
        Ok(record.projection.clone())
    }

    pub(super) fn projection(&mut self, id: Uuid, now_ms: u64) -> Option<MutationProjection> {
        self.prune(now_ms);
        self.records
            .get(&id)
            .map(|record| record.projection.clone())
    }

    fn prune(&mut self, now_ms: u64) {
        self.records.retain(|_, record| {
            record
                .terminal_at
                .is_none_or(|terminal| now_ms.saturating_sub(terminal) < TERMINAL_TTL_MS)
        });
    }
}

fn matching<T>(
    record: &ActorMutationRecord<T>,
    receipt: &MutationReceipt,
    snapshot: &SelectionSnapshot,
    transformed: &str,
) -> bool {
    record.projection.receipt == *receipt
        && record.projection.snapshot.same_target(snapshot)
        && record.projection.original_text == snapshot.text
        && record.projection.transformed_text == transformed
}

#[cfg(test)]
#[path = "macos_mutation_ledger_tests.rs"]
mod tests;
