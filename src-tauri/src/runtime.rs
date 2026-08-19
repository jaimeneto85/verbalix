use crate::commands_live::make_live_emitter;
use crate::{
    application::{
        AudioCapturePort, AudioPreviewPort, AudioStreamPort, EnrollmentSession,
        JsonSettingsRepository, KeychainSessionRepository, LiveInterpretationCoordinator,
        OnAirGuard, PlaybackRouter, PreferencesSyncStore, PublicBackendConfig,
        RemoteAuthRepository, RemoteHistoryRepository, RemotePreferencesRepository,
        RemoteVoiceEnrollment, RemoteVoicePipeline, RuntimePause, SelectionCoordinator,
        VirtualMicDevicePort, VirtualMicOutputPort, VirtualMicStatus, VoiceEnrollmentPort,
        VoicePipelinePort,
    },
    domain::{AppSettings, SelectionEvent, SettingsRepository, VerbalixError},
    platform::{MacAccessibility, SystemClipboard, TauriOverlay},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::{thread, time::Duration};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
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
    pub virtual_mic_device: Arc<dyn VirtualMicDevicePort>,
}

impl AppRuntime {
    pub fn virtual_mic_status(&self) -> VirtualMicStatus {
        self.virtual_mic_device.status()
    }
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

pub(crate) struct LiveComponents {
    pub coordinator: Arc<LiveInterpretationCoordinator>,
    pub virtual_mic_device: Arc<dyn VirtualMicDevicePort>,
}

pub(crate) fn build_live_coordinator(
    base_url: &str,
    anonymous_key: &str,
    stream: Arc<dyn AudioStreamPort>,
    pause: Arc<RuntimePause>,
    app: AppHandle,
) -> LiveComponents {
    let pipeline =
        Arc::new(RemoteVoicePipeline::new(base_url, anonymous_key)) as Arc<dyn VoicePipelinePort>;

    #[cfg(target_os = "macos")]
    let speaker = Arc::new(crate::platform::MacAudioPlayback::new()) as Arc<dyn AudioPreviewPort>;

    #[cfg(not(target_os = "macos"))]
    let speaker = Arc::new(crate::platform::StubAudioPlayback) as Arc<dyn AudioPreviewPort>;

    #[cfg(target_os = "macos")]
    let (virtual_mic_device, virtual_mic_output) = {
        let device =
            Arc::new(crate::platform::MacVirtualMicDevice::new()) as Arc<dyn VirtualMicDevicePort>;
        let output =
            Arc::new(crate::platform::MacVirtualMicOutput::new()) as Arc<dyn VirtualMicOutputPort>;
        (device, output)
    };

    #[cfg(not(target_os = "macos"))]
    let (virtual_mic_device, virtual_mic_output) = (
        Arc::new(crate::platform::StubVirtualMicDevice) as Arc<dyn VirtualMicDevicePort>,
        Arc::new(crate::platform::StubVirtualMicOutput) as Arc<dyn VirtualMicOutputPort>,
    );

    let route = Arc::new(AtomicBool::new(false));

    {
        let route_watch = Arc::clone(&route);
        let vmic_watch = Arc::clone(&virtual_mic_output);
        let app_watch = app.clone();
        virtual_mic_device.watch(Box::new(move |status| {
            if status != VirtualMicStatus::Installed && route_watch.load(Ordering::Relaxed) {
                route_watch.store(false, Ordering::Relaxed);
                vmic_watch.close();
                let _ = app_watch.emit(
                    "virtual-mic-status",
                    serde_json::json!({ "status": "notInstalled" }),
                );
            }
        }));
    }

    let router = Arc::new(PlaybackRouter::new(
        speaker,
        Arc::clone(&virtual_mic_output),
        Arc::clone(&route),
    )) as Arc<dyn AudioPreviewPort>;

    let on_live_event = make_live_emitter(app);

    let coordinator = Arc::new(LiveInterpretationCoordinator::new(
        pipeline,
        stream,
        router,
        pause,
        on_live_event,
        virtual_mic_output,
        route,
    ));

    LiveComponents {
        coordinator,
        virtual_mic_device,
    }
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
