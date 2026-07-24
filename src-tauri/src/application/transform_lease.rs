use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc,
};
use uuid::Uuid;

const ACTIVE: u8 = 0;
const CLAIMED: u8 = 1;
const CANCELLED: u8 = 2;

pub(crate) type PublicationGuard = Arc<TransformLease>;

pub(crate) struct TransformLease {
    snapshot_id: Uuid,
    request_id: Uuid,
    write_phase: AtomicU8,
    visual_cancelled: AtomicBool,
}

impl std::fmt::Debug for TransformLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TransformLease")
            .field("snapshot_id", &self.snapshot_id)
            .field("request_id", &self.request_id)
            .finish_non_exhaustive()
    }
}

impl PartialEq for TransformLease {
    fn eq(&self, other: &Self) -> bool {
        self.snapshot_id == other.snapshot_id && self.request_id == other.request_id
    }
}

impl Eq for TransformLease {}

impl TransformLease {
    pub(crate) fn new(snapshot_id: Uuid, request_id: Uuid) -> Self {
        Self {
            snapshot_id,
            request_id,
            write_phase: AtomicU8::new(ACTIVE),
            visual_cancelled: AtomicBool::new(false),
        }
    }

    pub(crate) fn owns(&self, snapshot_id: Uuid, request_id: Uuid) -> bool {
        self.snapshot_id == snapshot_id && self.request_id == request_id
    }

    pub(crate) fn cancel(&self) {
        self.visual_cancelled.store(true, Ordering::Release);
        let _ = self.write_phase.compare_exchange(
            ACTIVE,
            CANCELLED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    pub(crate) fn try_claim_write(&self) -> bool {
        self.write_phase
            .compare_exchange(ACTIVE, CLAIMED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn may_publish(&self) -> bool {
        !self.visual_cancelled.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_revokes_unclaimed_write_and_visual_publication() {
        let lease = TransformLease::new(Uuid::new_v4(), Uuid::new_v4());
        lease.cancel();

        assert!(!lease.try_claim_write());
        assert!(!lease.may_publish());
    }

    #[test]
    fn claimed_write_is_not_revoked_but_visual_publication_is() {
        let lease = TransformLease::new(Uuid::new_v4(), Uuid::new_v4());
        assert!(lease.try_claim_write());
        lease.cancel();

        assert!(!lease.try_claim_write());
        assert!(!lease.may_publish());
    }
}
