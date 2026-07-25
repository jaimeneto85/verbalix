use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationReceipt {
    pub id: Uuid,
    pub snapshot_id: Uuid,
    pub request_id: Uuid,
}
