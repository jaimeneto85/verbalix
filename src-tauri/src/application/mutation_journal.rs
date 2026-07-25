use super::{MutationReceipt, PublicationGuard, TransformLease};
use crate::domain::SelectionSnapshot;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

const JOURNAL_CAPACITY: usize = 64;

#[derive(Clone)]
pub(super) struct MutationRecord {
    pub(super) receipt: MutationReceipt,
    pub(super) snapshot: SelectionSnapshot,
    pub(super) transformed_text: String,
    pub(super) undo_lease: PublicationGuard,
    pub(super) restored: bool,
}

#[derive(Default)]
struct JournalState {
    records: HashMap<Uuid, MutationRecord>,
    order: VecDeque<Uuid>,
}

#[derive(Default)]
pub(super) struct MutationJournal {
    state: Mutex<JournalState>,
}

impl MutationJournal {
    pub(super) fn record(
        &self,
        receipt: MutationReceipt,
        snapshot: SelectionSnapshot,
        transformed_text: String,
    ) -> MutationRecord {
        let undo_lease = Arc::new(TransformLease::new(snapshot.id, receipt.request_id));
        let record = MutationRecord {
            receipt: receipt.clone(),
            snapshot,
            transformed_text,
            undo_lease,
            restored: false,
        };
        let mut state = self.lock();
        if !state.records.contains_key(&receipt.id) {
            state.order.push_back(receipt.id);
        }
        state.records.insert(receipt.id, record.clone());
        while state.records.len() > JOURNAL_CAPACITY {
            if let Some(oldest) = state.order.pop_front() {
                state.records.remove(&oldest);
            }
        }
        record
    }

    pub(super) fn get(&self, id: Uuid) -> Option<MutationRecord> {
        self.lock().records.get(&id).cloned()
    }

    pub(super) fn mark_restored(&self, id: Uuid) {
        if let Some(record) = self.lock().records.get_mut(&id) {
            record.restored = true;
        }
    }

    #[cfg(test)]
    pub(super) fn contains(&self, id: Uuid) -> bool {
        self.lock().records.contains_key(&id)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, JournalState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Rect, TextRange};

    fn snapshot() -> SelectionSnapshot {
        SelectionSnapshot::new(
            1,
            "pid:1".to_owned(),
            "before".to_owned(),
            TextRange {
                location: 0,
                length: 6,
            },
            Rect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            true,
        )
    }

    #[test]
    fn receipt_is_recorded_before_visual_state_and_survives_restore_marking() {
        let journal = MutationJournal::default();
        let selected = snapshot();
        let receipt = MutationReceipt {
            id: Uuid::new_v4(),
            snapshot_id: selected.id,
            request_id: Uuid::new_v4(),
        };
        let record = journal.record(receipt.clone(), selected, "after".to_owned());
        assert!(journal.contains(receipt.id));
        assert!(!record.restored);
        journal.mark_restored(receipt.id);
        assert!(journal.get(receipt.id).unwrap().restored);
    }

    #[test]
    fn duplicate_transformed_text_remains_addressable_by_exact_receipt() {
        let journal = MutationJournal::default();
        let first_snapshot = snapshot();
        let mut second_snapshot = snapshot();
        second_snapshot.id = Uuid::new_v4();
        let first = MutationReceipt {
            id: Uuid::new_v4(),
            snapshot_id: first_snapshot.id,
            request_id: Uuid::new_v4(),
        };
        let second = MutationReceipt {
            id: Uuid::new_v4(),
            snapshot_id: second_snapshot.id,
            request_id: Uuid::new_v4(),
        };
        journal.record(first.clone(), first_snapshot, "same result".to_owned());
        journal.record(second.clone(), second_snapshot, "same result".to_owned());

        journal.mark_restored(first.id);

        assert!(journal.get(first.id).unwrap().restored);
        assert!(!journal.get(second.id).unwrap().restored);
        assert_eq!(journal.get(second.id).unwrap().receipt, second);
    }

    #[test]
    fn capacity_evicts_oldest_receipt_deterministically() {
        let journal = MutationJournal::default();
        let mut ids = Vec::new();
        for index in 0..=JOURNAL_CAPACITY {
            let mut selected = snapshot();
            selected.id = Uuid::from_u128(index as u128 + 1);
            let receipt = MutationReceipt {
                id: Uuid::from_u128(index as u128 + 101),
                snapshot_id: selected.id,
                request_id: Uuid::from_u128(index as u128 + 201),
            };
            ids.push(receipt.id);
            journal.record(receipt, selected, format!("result-{index}"));
        }

        assert!(journal.get(ids[0]).is_none());
        for id in &ids[1..] {
            assert!(journal.get(*id).is_some());
        }
    }
}
