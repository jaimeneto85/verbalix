#[cfg(target_os = "macos")]
mod audio_capture;
#[cfg(target_os = "macos")]
pub mod audio_permission;
#[cfg(target_os = "macos")]
mod audio_playback;
#[cfg(target_os = "macos")]
mod audio_processing;
pub mod audio_resample;
pub mod audio_wav;
mod causal_epoch;
#[cfg(target_os = "macos")]
mod causal_registry;
mod clipboard;
mod note_result;
mod overlay;
mod overlay_dispatcher;
mod overlay_execution;
mod overlay_geometry;
mod overlay_publication;
mod overlay_readiness;
mod overlay_window;
#[cfg(test)]
mod overlay_window_tests;

#[cfg(target_os = "macos")]
mod macos_accessibility;
#[cfg(target_os = "macos")]
mod macos_attribute;
#[cfg(target_os = "macos")]
mod macos_ax;
#[cfg(target_os = "macos")]
mod macos_ax_actor;
#[cfg(target_os = "macos")]
mod macos_ax_actor_observation;
#[cfg(target_os = "macos")]
mod macos_ax_actor_reconciliation;
#[cfg(target_os = "macos")]
mod macos_ax_actor_state;
#[cfg(target_os = "macos")]
mod macos_ax_target;
#[cfg(target_os = "macos")]
mod macos_classic_range;
#[cfg(target_os = "macos")]
mod macos_element_token;
#[cfg(target_os = "macos")]
pub(crate) mod macos_focus;
#[cfg(all(target_os = "macos", test))]
mod macos_focus_tests;
#[cfg(target_os = "macos")]
mod macos_geometry;
mod macos_mutation_ledger;
#[cfg(target_os = "macos")]
mod macos_observer;
#[cfg(target_os = "macos")]
mod macos_overlay_panel;
#[cfg(target_os = "macos")]
mod macos_replace;
#[cfg(target_os = "macos")]
mod macos_restore;
#[cfg(target_os = "macos")]
mod macos_selection;
#[cfg(target_os = "macos")]
mod macos_selection_identity;
#[cfg(target_os = "macos")]
mod macos_selection_revalidation;
#[cfg(target_os = "macos")]
mod macos_self_notification_phase;
#[cfg(target_os = "macos")]
mod macos_text_marker;
#[cfg(target_os = "macos")]
mod macos_text_role;
#[cfg(target_os = "macos")]
mod macos_value_range;
#[cfg(target_os = "macos")]
mod macos_write_authorization;
#[cfg(target_os = "macos")]
mod macos_write_boundary;

#[cfg(target_os = "macos")]
mod virtual_mic;
pub mod virtual_mic_constants;
#[cfg(target_os = "macos")]
mod virtual_mic_output;

pub use clipboard::SystemClipboard;
pub use note_result::NoteResultPayload;
#[cfg(target_os = "macos")]
pub use overlay::install_mouse_dismiss_monitor;
pub use overlay::TauriOverlay;
pub(crate) use overlay_readiness::OverlaySurface;
pub(crate) use overlay_window::is_current_caller;

#[cfg(target_os = "macos")]
pub use audio_capture::MacAudioCapture;
#[cfg(target_os = "macos")]
pub use audio_permission::{microphone_permission_status, request_microphone_permission};
#[cfg(target_os = "macos")]
pub use audio_playback::MacAudioPlayback;
#[cfg(target_os = "macos")]
pub use macos_accessibility::MacAccessibility;
#[cfg(target_os = "macos")]
pub use virtual_mic::MacVirtualMicDevice;
#[cfg(target_os = "macos")]
pub use virtual_mic_output::MacVirtualMicOutput;

#[cfg(not(target_os = "macos"))]
pub use unsupported::MacAccessibility;

#[cfg(not(target_os = "macos"))]
mod unsupported {
    use crate::{
        application::SelectionPort,
        domain::{SelectionSnapshot, VerbalixError},
    };

    pub struct MacAccessibility;

    impl MacAccessibility {
        pub fn new() -> Self {
            Self
        }
    }

    impl SelectionPort for MacAccessibility {
        fn permission_granted(&self, _prompt: bool) -> bool {
            false
        }

        fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
            Err(VerbalixError::UnsupportedPlatform)
        }

        fn replace(&self, _expected: &SelectionSnapshot, _text: &str) -> Result<(), VerbalixError> {
            Err(VerbalixError::UnsupportedPlatform)
        }

        fn restore(
            &self,
            _expected: &SelectionSnapshot,
            _transformed_text: &str,
        ) -> Result<(), VerbalixError> {
            Err(VerbalixError::UnsupportedPlatform)
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub struct StubAudioCapture;

#[cfg(not(target_os = "macos"))]
pub struct StubAudioStream;

#[cfg(not(target_os = "macos"))]
pub struct StubAudioPlayback;

#[cfg(not(target_os = "macos"))]
impl crate::application::AudioCapturePort for StubAudioCapture {
    fn start(&self) -> Result<(), crate::domain::VerbalixError> {
        Err(crate::domain::VerbalixError::UnsupportedPlatform)
    }

    fn stop(&self) -> Result<crate::domain::EnrollmentSample, crate::domain::VerbalixError> {
        Err(crate::domain::VerbalixError::UnsupportedPlatform)
    }

    fn cancel(&self) {}

    fn level(&self) -> f32 {
        0.0
    }

    fn permission_status(&self) -> crate::domain::MicrophonePermission {
        crate::domain::MicrophonePermission::Denied
    }
}

#[cfg(not(target_os = "macos"))]
pub fn microphone_permission_status() -> crate::domain::MicrophonePermission {
    crate::domain::MicrophonePermission::Denied
}

#[cfg(not(target_os = "macos"))]
pub fn request_microphone_permission<
    F: FnOnce(crate::domain::MicrophonePermission) + Send + 'static,
>(
    _callback: F,
) {
}

#[cfg(not(target_os = "macos"))]
impl crate::application::AudioStreamPort for StubAudioStream {
    fn start_stream(
        &self,
        _sink: Box<dyn Fn(Vec<f32>, u32, u16) + Send + Sync + 'static>,
        _error_sink: Box<dyn Fn() + Send + Sync + 'static>,
    ) -> Result<(), crate::domain::VerbalixError> {
        Err(crate::domain::VerbalixError::UnsupportedPlatform)
    }

    fn stop_stream(&self) {}
}

#[cfg(not(target_os = "macos"))]
impl crate::application::AudioPreviewPort for StubAudioPlayback {
    fn play(&self, _wav_bytes: Vec<u8>) -> Result<(), crate::domain::VerbalixError> {
        Err(crate::domain::VerbalixError::UnsupportedPlatform)
    }

    fn stop(&self) {}
}

#[cfg(not(target_os = "macos"))]
pub struct StubVirtualMicDevice;

#[cfg(not(target_os = "macos"))]
pub struct StubVirtualMicOutput;

#[cfg(not(target_os = "macos"))]
impl crate::application::VirtualMicDevicePort for StubVirtualMicDevice {
    fn status(&self) -> crate::application::VirtualMicStatus {
        crate::application::VirtualMicStatus::NotInstalled
    }

    fn watch(&self, _on_change: Box<dyn Fn(crate::application::VirtualMicStatus) + Send + Sync>) {}
}

#[cfg(not(target_os = "macos"))]
impl crate::application::VirtualMicOutputPort for StubVirtualMicOutput {
    fn open(&self) -> Result<(), crate::domain::VerbalixError> {
        Err(crate::domain::VerbalixError::VirtualMicUnavailable)
    }

    fn enqueue(&self, _samples_48k: Vec<f32>, _channels: u16) {}

    fn close(&self) {}

    fn metrics(&self) -> crate::application::VirtualMicMetrics {
        crate::application::VirtualMicMetrics {
            buffer_depth: 0,
            underruns: 0,
        }
    }
}
