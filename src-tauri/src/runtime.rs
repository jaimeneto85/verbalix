use crate::{
    application::{
        JsonSettingsRepository, KeychainSessionRepository, PreferencesSyncStore,
        PublicBackendConfig, RemoteAuthRepository, RemoteHistoryRepository,
        RemotePreferencesRepository, RuntimePause, SelectionCoordinator,
    },
    domain::{AppSettings, SelectionEvent, SettingsRepository, VerbalixError},
    platform::{MacAccessibility, SystemClipboard, TauriOverlay},
};
use std::sync::{Arc, Mutex};
use std::{thread, time::Duration};

pub(crate) struct AppRuntime {
    pub coordinator: Arc<SelectionCoordinator>,
    pub overlay: Arc<TauriOverlay>,
    pub selection: Arc<MacAccessibility>,
    pub settings: Arc<JsonSettingsRepository>,
    pub preferences_sync: Arc<PreferencesSyncStore>,
    pub synced_settings: Mutex<Option<AppSettings>>,
    pub session: Arc<KeychainSessionRepository>,
    pub clipboard: Arc<SystemClipboard>,
    pub history: Arc<RemoteHistoryRepository>,
    pub auth: Arc<RemoteAuthRepository>,
    pub backend_config: PublicBackendConfig,
    pub pause: RuntimePause,
    pub remote_preferences: Option<Arc<RemotePreferencesRepository>>,
}

pub(crate) fn start_selection_observer(runtime: Arc<AppRuntime>) {
    thread::spawn(move || {
        let mut candidate_id = None;
        loop {
            let settings = runtime.settings.load().unwrap_or_default();
            let result = runtime.pause.run_polling(settings.automatic_toolbar, || {
                crate::diagnostics::detection("polling");
                match runtime.coordinator.refresh_selection() {
                    Ok(Some(snapshot)) if candidate_id != Some(snapshot.id) => {
                        candidate_id = Some(snapshot.id);
                        thread::sleep(Duration::from_millis(150));
                        if !runtime.pause.is_paused() {
                            let _ = runtime
                                .coordinator
                                .dispatch(SelectionEvent::DebounceElapsed(snapshot.id));
                        }
                    }
                    Err(error @ VerbalixError::SelectionUnavailable)
                    | Err(error @ VerbalixError::ProtectedField)
                    | Err(error @ VerbalixError::PermissionDenied) => {
                        crate::diagnostics::capture_failure("polling", &error);
                        candidate_id = None;
                        let _ = runtime
                            .coordinator
                            .dispatch(SelectionEvent::TransientInvalidated);
                    }
                    _ => {}
                }
            });
            if result.is_none() {
                candidate_id = None;
            }
            thread::sleep(Duration::from_millis(120));
        }
    });
}
