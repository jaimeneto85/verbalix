use super::SelectionCoordinator;
use crate::{
    application::{MutationReceipt, MutationStatus, PublicationGuard},
    domain::{SelectionSnapshot, VerbalixError},
};
use uuid::Uuid;

impl SelectionCoordinator {
    pub(super) fn record_confirmed_mutation(
        &self,
        receipt: MutationReceipt,
        snapshot: SelectionSnapshot,
        result: String,
    ) -> Result<super::mutation_journal::MutationRecord, VerbalixError> {
        let recovered = self.selection.reconcile_mutation(receipt.id)?;
        if let Some(record) = recovered {
            if record.receipt != receipt
                || record.status != MutationStatus::Confirmed
                || !record.snapshot.same_target(&snapshot)
                || record.original_text != snapshot.text
                || record.transformed_text != result
                || record.strategy != snapshot.extraction_strategy
                || record.target_snapshot_id != snapshot.id
            {
                return Err(VerbalixError::LocalFailure);
            }
        }
        Ok(self.mutation_journal.record(receipt, snapshot, result))
    }

    pub(super) fn execute_replace(
        &self,
        snapshot: &SelectionSnapshot,
        result: &str,
        lease: &PublicationGuard,
    ) -> Result<MutationReceipt, VerbalixError> {
        let mutation_id = Uuid::new_v4();
        match self
            .selection
            .replace_guarded_with_id(snapshot, result, lease, mutation_id)
        {
            Ok(receipt) => Ok(receipt),
            Err(error) => self
                .selection
                .reconcile_mutation(mutation_id)?
                .filter(|record| {
                    record.status == MutationStatus::Confirmed
                        && record.snapshot.same_target(snapshot)
                        && record.transformed_text == result
                })
                .map(|record| record.receipt)
                .ok_or(error),
        }
    }
}
