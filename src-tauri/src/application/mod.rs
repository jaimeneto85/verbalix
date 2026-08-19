mod ai_readiness;
mod auth_refresh;
mod coordinator;
mod coordinator_commit;
mod coordinator_mutation;
mod coordinator_presentation;
mod coordinator_transform;
mod enrollment_session;
pub(crate) mod live_interpretation;
pub(crate) mod live_queue;
pub(crate) mod live_worker;
mod mutation;
mod mutation_journal;
mod ports;
pub(crate) mod preferences_sync_store;
pub(crate) mod remote_preferences;
mod runtime_pause;
mod settings_file;
mod supabase;
mod transform_lease;
mod voice_enrollment;
pub(crate) mod voice_pipeline;

pub use ai_readiness::{
    classify_refresh_failure, evaluate_ai_readiness, AiReadiness, AiReadinessStatus,
    PublicBackendConfig, RefreshFailureRoute,
};
pub use auth_refresh::RemoteAuthRepository;
pub use coordinator::SelectionCoordinator;
pub use enrollment_session::EnrollmentSession;
pub use live_interpretation::{LiveEventFn, LiveInterpretationCoordinator};
pub use mutation::{MutationProjection, MutationReceipt, MutationStatus};
pub use ports::{
    AudioCapturePort, AudioPreviewPort, AudioStreamPort, ClipboardPort, OverlayPort, SelectionPort,
    VirtualMicDevicePort, VirtualMicMetrics, VirtualMicOutputPort, VirtualMicStatus,
    VoiceEnrollmentPort, VoicePipelinePort,
};
pub use preferences_sync_store::{epoch_secs_now, PreferencesSyncStore};
pub use remote_preferences::{merge_preferences, RemotePreferencesRepository};
#[cfg(test)]
pub(crate) use runtime_pause::{set_test_clock_ms, test_clock_ms};
pub use runtime_pause::{OnAirGuard, RuntimePause};
pub use settings_file::JsonSettingsRepository;
pub use supabase::{
    HistoryItem, KeychainSessionRepository, RemoteHistoryRepository, RemoteTransformer,
    SessionRepository, StoredSession,
};
pub(crate) use transform_lease::{PublicationGuard, PublicationPermit, TransformLease};
pub use voice_enrollment::RemoteVoiceEnrollment;
pub use voice_pipeline::RemoteVoicePipeline;
