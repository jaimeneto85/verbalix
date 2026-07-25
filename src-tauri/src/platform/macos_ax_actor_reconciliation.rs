use super::{
    macos_ax_actor_state::ActorState,
    macos_mutation_ledger::{ReplaceTerminalOutcome, RestoreTerminalOutcome},
    macos_selection_revalidation,
};
use crate::application::{MutationProjection, MutationStatus};
use uuid::Uuid;

impl ActorState {
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
                .map(|current| {
                    let expected_location = record.projection.snapshot.range.location;
                    let current_location = current.range.location as i64;
                    let status = if restoring
                        && current.text == record.projection.original_text
                        && current_location == expected_location
                        && current.range.length as i64 == record.projection.snapshot.range.length
                    {
                        MutationStatus::Restored
                    } else if restoring
                        && current.text == record.projection.transformed_text
                        && current_location == expected_location
                        && current.range.length as usize
                            == record.projection.transformed_text.encode_utf16().count()
                    {
                        MutationStatus::RestoreRejected
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
                    } else if restoring {
                        MutationStatus::RestoreIndeterminate
                    } else {
                        MutationStatus::Indeterminate
                    };
                    (restoring, status)
                })
                .unwrap_or((
                    restoring,
                    if restoring {
                        MutationStatus::RestoreRejected
                    } else {
                        MutationStatus::Rejected
                    },
                ))
            })
        });
        if let Some((restoring, status)) = recovery {
            if restoring {
                let outcome = match status {
                    MutationStatus::Restored => RestoreTerminalOutcome::Restored,
                    MutationStatus::RestoreRejected => RestoreTerminalOutcome::Rejected,
                    MutationStatus::RestoreIndeterminate => RestoreTerminalOutcome::Indeterminate,
                    _ => return self.mutations.projection(id, self.now()),
                };
                let _ = self.mutations.reconcile_restore(id, outcome, self.now());
            } else {
                let outcome = match status {
                    MutationStatus::Confirmed => ReplaceTerminalOutcome::Confirmed,
                    MutationStatus::Rejected => ReplaceTerminalOutcome::Rejected,
                    MutationStatus::Indeterminate => ReplaceTerminalOutcome::Indeterminate,
                    _ => return self.mutations.projection(id, self.now()),
                };
                let _ = self.mutations.reconcile_replace(id, outcome, self.now());
            }
        }
        self.mutations.projection(id, self.now())
    }
}
