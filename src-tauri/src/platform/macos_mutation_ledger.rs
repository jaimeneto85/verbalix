use crate::{
    application::{MutationProjection, MutationReceipt, MutationStatus},
    domain::{SelectionSnapshot, VerbalixError},
};
use std::collections::HashMap;
use uuid::Uuid;

const TERMINAL_TTL_MS: u64 = 600_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReplaceTerminalOutcome {
    Confirmed,
    Rejected,
    Indeterminate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RestoreTerminalOutcome {
    Restored,
    Rejected,
    Indeterminate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalPhase {
    FinishReplace,
    ReconcileReplace,
    FinishRestore,
    ReconcileRestore,
}

pub(super) struct ActorMutationRecord<T> {
    pub(super) projection: MutationProjection,
    pub(super) target: T,
    restore_attempted: bool,
    terminal_at: Option<u64>,
    terminal_phase: Option<TerminalPhase>,
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
                restore_attempted: false,
                terminal_at: None,
                terminal_phase: None,
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

    pub(super) fn finish_replace(
        &mut self,
        id: Uuid,
        outcome: ReplaceTerminalOutcome,
        now_ms: u64,
    ) -> Result<MutationProjection, VerbalixError> {
        let record = self
            .records
            .get_mut(&id)
            .ok_or(VerbalixError::LocalFailure)?;
        apply_replace_outcome(
            record,
            MutationStatus::Prepared,
            TerminalPhase::FinishReplace,
            outcome,
            now_ms,
        )
    }

    pub(super) fn reconcile_replace(
        &mut self,
        id: Uuid,
        outcome: ReplaceTerminalOutcome,
        now_ms: u64,
    ) -> Result<MutationProjection, VerbalixError> {
        let record = self
            .records
            .get_mut(&id)
            .ok_or(VerbalixError::LocalFailure)?;
        apply_replace_outcome(
            record,
            MutationStatus::Indeterminate,
            TerminalPhase::ReconcileReplace,
            outcome,
            now_ms,
        )
    }

    pub(super) fn begin_restore(
        &mut self,
        id: Uuid,
        now_ms: u64,
    ) -> Result<MutationProjection, VerbalixError> {
        self.prune(now_ms);
        let record = self
            .records
            .get_mut(&id)
            .ok_or(VerbalixError::StaleSelection)?;
        if record.projection.status != MutationStatus::Confirmed || record.restore_attempted {
            return Err(VerbalixError::StaleSelection);
        }
        record.restore_attempted = true;
        record.projection.status = MutationStatus::RestorePrepared;
        record.terminal_at = None;
        record.terminal_phase = None;
        Ok(record.projection.clone())
    }

    pub(super) fn finish_restore(
        &mut self,
        id: Uuid,
        outcome: RestoreTerminalOutcome,
        now_ms: u64,
    ) -> Result<MutationProjection, VerbalixError> {
        let record = self
            .records
            .get_mut(&id)
            .ok_or(VerbalixError::StaleSelection)?;
        apply_restore_outcome(
            record,
            MutationStatus::RestorePrepared,
            TerminalPhase::FinishRestore,
            outcome,
            now_ms,
        )
    }

    pub(super) fn reconcile_restore(
        &mut self,
        id: Uuid,
        outcome: RestoreTerminalOutcome,
        now_ms: u64,
    ) -> Result<MutationProjection, VerbalixError> {
        let record = self
            .records
            .get_mut(&id)
            .ok_or(VerbalixError::StaleSelection)?;
        apply_restore_outcome(
            record,
            MutationStatus::RestoreIndeterminate,
            TerminalPhase::ReconcileRestore,
            outcome,
            now_ms,
        )
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

fn apply_replace_outcome<T>(
    record: &mut ActorMutationRecord<T>,
    expected: MutationStatus,
    phase: TerminalPhase,
    outcome: ReplaceTerminalOutcome,
    now_ms: u64,
) -> Result<MutationProjection, VerbalixError> {
    let status = match outcome {
        ReplaceTerminalOutcome::Confirmed => MutationStatus::Confirmed,
        ReplaceTerminalOutcome::Rejected => MutationStatus::Rejected,
        ReplaceTerminalOutcome::Indeterminate => MutationStatus::Indeterminate,
    };
    if record.projection.status == status && record.terminal_phase == Some(phase) {
        return Ok(record.projection.clone());
    }
    if record.projection.status != expected {
        return Err(VerbalixError::StaleSelection);
    }
    record.projection.status = status;
    record.terminal_phase = Some(phase);
    record.terminal_at =
        matches!(status, MutationStatus::Confirmed | MutationStatus::Rejected).then_some(now_ms);
    Ok(record.projection.clone())
}

fn apply_restore_outcome<T>(
    record: &mut ActorMutationRecord<T>,
    expected: MutationStatus,
    phase: TerminalPhase,
    outcome: RestoreTerminalOutcome,
    now_ms: u64,
) -> Result<MutationProjection, VerbalixError> {
    let status = match outcome {
        RestoreTerminalOutcome::Restored => MutationStatus::Restored,
        RestoreTerminalOutcome::Rejected => MutationStatus::RestoreRejected,
        RestoreTerminalOutcome::Indeterminate => MutationStatus::RestoreIndeterminate,
    };
    if record.projection.status == status && record.terminal_phase == Some(phase) {
        return Ok(record.projection.clone());
    }
    if record.projection.status != expected {
        return Err(VerbalixError::StaleSelection);
    }
    record.projection.status = status;
    record.terminal_phase = Some(phase);
    record.terminal_at = matches!(
        status,
        MutationStatus::Restored | MutationStatus::RestoreRejected
    )
    .then_some(now_ms);
    Ok(record.projection.clone())
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

#[cfg(test)]
#[path = "macos_mutation_ledger_transition_tests.rs"]
mod transition_tests;
