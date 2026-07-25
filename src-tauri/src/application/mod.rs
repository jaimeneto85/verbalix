mod ai_readiness;
mod auth_refresh;
mod coordinator;
mod coordinator_commit;
mod coordinator_presentation;
mod coordinator_transform;
mod mutation;
mod mutation_journal;
mod ports;
mod runtime_pause;
mod settings_file;
mod supabase;
mod transform_lease;

pub use ai_readiness::{
    classify_refresh_failure, evaluate_ai_readiness, AiReadiness, AiReadinessStatus,
    PublicBackendConfig, RefreshFailureRoute,
};
pub use auth_refresh::RemoteAuthRepository;
pub use coordinator::SelectionCoordinator;
pub use mutation::MutationReceipt;
pub use ports::{ClipboardPort, OverlayPort, SelectionPort};
pub use runtime_pause::RuntimePause;
pub use settings_file::JsonSettingsRepository;
pub use supabase::{
    HistoryItem, KeychainSessionRepository, RemoteHistoryRepository, RemoteTransformer,
    SessionRepository, StoredSession,
};
pub(crate) use transform_lease::{PublicationGuard, PublicationPermit, TransformLease};
