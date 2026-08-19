use crate::{
    application::{
        AudioCapturePort, AudioPreviewPort, AudioStreamPort, EnrollmentSession,
        JsonSettingsRepository, KeychainSessionRepository, LiveInterpretationCoordinator,
        OnAirGuard, PreferencesSyncStore, PublicBackendConfig, RemoteAuthRepository,
        RemoteHistoryRepository, RemotePreferencesRepository, RemoteVoiceEnrollment,
        RemoteVoicePipeline, RuntimePause, SelectionCoordinator, VoiceEnrollmentPort,
        VoicePipelinePort,
    },
    domain::{AppSettings, SelectionEvent, SettingsRepository, VerbalixError},
    platform::{MacAccessibility, SystemClipboard, TauriOverlay},
};
use std::sync::{Arc, Mutex};
use std::{thread, time::Duration};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

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
    pub pause: Arc<RuntimePause>,
    pub remote_preferences: Option<Arc<RemotePreferencesRepository>>,
    pub voice_enrollment: Arc<dyn VoiceEnrollmentPort>,
    pub audio_capture: Arc<dyn AudioCapturePort>,
    pub enrollment_session: Arc<EnrollmentSession>,
    pub live_coordinator: Arc<LiveInterpretationCoordinator>,
    pub on_air_guard: Mutex<Option<OnAirGuard>>,
}

pub(crate) struct VoiceComponents {
    pub enrollment: Arc<dyn VoiceEnrollmentPort>,
    pub capture: Arc<dyn AudioCapturePort>,
    pub stream: Arc<dyn AudioStreamPort>,
    pub session: Arc<EnrollmentSession>,
}

pub(crate) fn build_voice_components(base_url: &str, anonymous_key: &str) -> VoiceComponents {
    let enrollment = Arc::new(RemoteVoiceEnrollment::new(base_url, anonymous_key))
        as Arc<dyn VoiceEnrollmentPort>;

    #[cfg(target_os = "macos")]
    let (capture, stream) = {
        let mac = Arc::new(crate::platform::MacAudioCapture::new());
        (
            mac.clone() as Arc<dyn AudioCapturePort>,
            mac as Arc<dyn AudioStreamPort>,
        )
    };

    #[cfg(not(target_os = "macos"))]
    let (capture, stream) = (
        Arc::new(crate::platform::StubAudioCapture) as Arc<dyn AudioCapturePort>,
        Arc::new(crate::platform::StubAudioStream) as Arc<dyn AudioStreamPort>,
    );

    VoiceComponents {
        enrollment,
        capture,
        stream,
        session: Arc::new(EnrollmentSession::new()),
    }
}

pub(crate) fn build_live_coordinator(
    base_url: &str,
    anonymous_key: &str,
    stream: Arc<dyn AudioStreamPort>,
    pause: Arc<RuntimePause>,
) -> Arc<LiveInterpretationCoordinator> {
    let pipeline = Arc::new(RemoteVoicePipeline::new(base_url, anonymous_key))
        as Arc<dyn VoicePipelinePort>;

    #[cfg(target_os = "macos")]
    let playback = Arc::new(crate::platform::MacAudioPlayback::new()) as Arc<dyn AudioPreviewPort>;

    #[cfg(not(target_os = "macos"))]
    let playback = Arc::new(crate::platform::StubAudioPlayback) as Arc<dyn AudioPreviewPort>;

    Arc::new(LiveInterpretationCoordinator::new(
        pipeline, stream, playback, pause,
    ))
}

pub(crate) fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "Configurações", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pausar", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings, &pause, &quit])?;
    let pause_item = pause.clone();
    TrayIconBuilder::new()
        .tooltip("Verbalix")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "settings" => show_main_window(app, "tray"),
            "quit" => {
                crate::diagnostics::lifecycle("quit_requested", "tray");
                app.exit(0);
            }
            "pause" => {
                let runtime = app.state::<Arc<AppRuntime>>();
                let paused = runtime.pause.toggle();
                let _ = pause_item.set_text(if paused { "Retomar" } else { "Pausar" });
                if paused {
                    let _ = runtime.coordinator.dispatch(SelectionEvent::Invalidated);
                    if runtime.pause.is_on_air() {
                        let r = runtime.inner().clone();
                        thread::spawn(move || r.live_coordinator.leave_live());
                    }
                }
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub(crate) fn show_main_window(app: &AppHandle, origin: &'static str) {
    crate::diagnostics::lifecycle("show_requested", origin);
    let Some(window) = app.get_webview_window("main") else {
        crate::diagnostics::lifecycle("show_failed", "main_window_missing");
        return;
    };
    if window.show().and_then(|_| window.set_focus()).is_ok() {
        crate::diagnostics::lifecycle("shown", origin);
    } else {
        crate::diagnostics::lifecycle("show_failed", origin);
    }
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
                        if !runtime.pause.is_paused() && !runtime.pause.is_action_in_flight() {
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
