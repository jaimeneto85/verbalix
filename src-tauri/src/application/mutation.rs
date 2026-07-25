use crate::domain::{SelectionExtractionStrategy, SelectionSnapshot};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationReceipt {
    pub id: Uuid,
    pub snapshot_id: Uuid,
    pub request_id: Uuid,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum MutationStatus {
    Prepared,
    Confirmed,
    Rejected,
    Indeterminate,
}

#[derive(Clone)]
pub struct MutationProjection {
    pub receipt: MutationReceipt,
    pub snapshot: SelectionSnapshot,
    pub original_text: String,
    pub transformed_text: String,
    pub strategy: SelectionExtractionStrategy,
    pub target_snapshot_id: Uuid,
    pub status: MutationStatus,
}
