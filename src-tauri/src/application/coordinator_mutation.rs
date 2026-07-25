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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{OverlayPort, SelectionPort},
        domain::{
            AiProvider, Rect, SelectionExtractionStrategy, TextRange, TransformRequest,
            TransformResult,
        },
    };
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    struct LostResponseSelection {
        projection: Mutex<Option<crate::application::MutationProjection>>,
        setters: Mutex<Vec<Uuid>>,
    }

    impl SelectionPort for LostResponseSelection {
        fn permission_granted(&self, _prompt: bool) -> bool {
            true
        }

        fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
            Err(VerbalixError::SelectionUnavailable)
        }

        fn replace(&self, _expected: &SelectionSnapshot, _text: &str) -> Result<(), VerbalixError> {
            panic!("execute_replace must use caller-preallocated mutation ID")
        }

        fn replace_guarded_with_id(
            &self,
            expected: &SelectionSnapshot,
            text: &str,
            lease: &PublicationGuard,
            mutation_id: Uuid,
        ) -> Result<MutationReceipt, VerbalixError> {
            assert!(lease.try_claim_write());
            self.setters.lock().unwrap().push(mutation_id);
            let receipt = MutationReceipt {
                id: mutation_id,
                snapshot_id: expected.id,
                request_id: lease.request_id(),
            };
            *self.projection.lock().unwrap() = Some(crate::application::MutationProjection {
                receipt,
                snapshot: expected.clone(),
                original_text: expected.text.clone(),
                transformed_text: text.to_owned(),
                strategy: expected.extraction_strategy,
                target_snapshot_id: expected.id,
                status: MutationStatus::Confirmed,
            });
            Err(VerbalixError::LocalFailure)
        }

        fn restore(
            &self,
            _expected: &SelectionSnapshot,
            _transformed_text: &str,
        ) -> Result<(), VerbalixError> {
            Ok(())
        }

        fn reconcile_mutation(
            &self,
            mutation_id: Uuid,
        ) -> Result<Option<crate::application::MutationProjection>, VerbalixError> {
            Ok(self
                .projection
                .lock()
                .unwrap()
                .clone()
                .filter(|projection| projection.receipt.id == mutation_id))
        }
    }

    struct NoopOverlay;

    impl OverlayPort for NoopOverlay {
        fn show_toolbar(&self, _bounds: Rect) -> Result<(), VerbalixError> {
            Ok(())
        }
        fn show_note(&self, _bounds: Rect, _text: &str) -> Result<(), VerbalixError> {
            Ok(())
        }
        fn show_preview(
            &self,
            _bounds: Rect,
            _request_id: Uuid,
            _text: &str,
        ) -> Result<(), VerbalixError> {
            Ok(())
        }
        fn show_undo(&self, _bounds: Rect, _text: &str) -> Result<(), VerbalixError> {
            Ok(())
        }
        fn hide_all(&self) -> Result<(), VerbalixError> {
            Ok(())
        }
    }

    struct NoopProvider;

    #[async_trait]
    impl AiProvider for NoopProvider {
        async fn transform(
            &self,
            _request: &TransformRequest,
            _access_token: &str,
        ) -> Result<TransformResult, VerbalixError> {
            Err(VerbalixError::LocalFailure)
        }
    }

    fn snapshot() -> SelectionSnapshot {
        SelectionSnapshot::new(
            42,
            "pid:42".to_owned(),
            "before".to_owned(),
            TextRange {
                location: 1,
                length: 6,
            },
            Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            },
            true,
        )
        .with_extraction_strategy(SelectionExtractionStrategy::ValueRange)
    }

    #[test]
    fn lost_response_recovers_the_caller_preallocated_confirmed_mutation() {
        let selection = Arc::new(LostResponseSelection {
            projection: Mutex::new(None),
            setters: Mutex::new(Vec::new()),
        });
        let coordinator = SelectionCoordinator::new(
            selection.clone(),
            Arc::new(NoopOverlay),
            Arc::new(NoopProvider),
        );
        let selected = snapshot();
        let lease = Arc::new(crate::application::TransformLease::new(
            selected.id,
            Uuid::new_v4(),
        ));

        let recovered = coordinator
            .execute_replace(&selected, "after", &lease)
            .unwrap();

        assert_eq!(selection.setters.lock().unwrap().as_slice(), [recovered.id]);
        let projection = selection.reconcile_mutation(recovered.id).unwrap().unwrap();
        assert_eq!(projection.receipt, recovered);
        assert!(projection.status == MutationStatus::Confirmed);
        assert_eq!(projection.original_text, "before");
        assert_eq!(projection.transformed_text, "after");
    }
}
