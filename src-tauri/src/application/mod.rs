mod ai_readiness;
mod auth_refresh;
mod coordinator;
mod ports;
mod runtime_pause;
mod settings_file;
mod supabase;

pub use ai_readiness::{
    classify_refresh_failure, evaluate_ai_readiness, AiReadiness, AiReadinessStatus,
    PublicBackendConfig, RefreshFailureRoute,
};
pub use auth_refresh::RemoteAuthRepository;
pub use coordinator::SelectionCoordinator;
pub use ports::{ClipboardPort, OverlayPort, SelectionPort};
pub use runtime_pause::RuntimePause;
pub use settings_file::JsonSettingsRepository;
pub use supabase::{
    HistoryItem, KeychainSessionRepository, RemoteHistoryRepository, RemoteTransformer,
    SessionRepository, StoredSession,
};
