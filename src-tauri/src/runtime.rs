use crate::{
    application::{
        JsonSettingsRepository, KeychainSessionRepository, PublicBackendConfig,
        RemoteAuthRepository, RemoteHistoryRepository, RemotePreferencesRepository, RuntimePause,
        SelectionCoordinator,
    },
    platform::{MacAccessibility, SystemClipboard, TauriOverlay},
};
use std::sync::Arc;

pub(crate) struct AppRuntime {
    pub coordinator: Arc<SelectionCoordinator>,
    pub overlay: Arc<TauriOverlay>,
    pub selection: Arc<MacAccessibility>,
    pub settings: Arc<JsonSettingsRepository>,
    pub session: Arc<KeychainSessionRepository>,
    pub clipboard: Arc<SystemClipboard>,
    pub history: Arc<RemoteHistoryRepository>,
    pub auth: Arc<RemoteAuthRepository>,
    pub backend_config: PublicBackendConfig,
    pub pause: RuntimePause,
    pub remote_preferences: Option<Arc<RemotePreferencesRepository>>,
}
