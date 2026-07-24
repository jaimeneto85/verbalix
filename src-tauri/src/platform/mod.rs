mod clipboard;
mod note_result;
mod overlay;
mod overlay_dispatcher;
mod overlay_geometry;
mod overlay_readiness;
mod overlay_window;
#[cfg(test)]
mod overlay_window_tests;

#[cfg(target_os = "macos")]
mod macos_accessibility;
#[cfg(target_os = "macos")]
mod macos_ax;
#[cfg(target_os = "macos")]
mod macos_classic_range;
#[cfg(target_os = "macos")]
pub(crate) mod macos_focus;
#[cfg(all(target_os = "macos", test))]
mod macos_focus_tests;
#[cfg(target_os = "macos")]
mod macos_geometry;
#[cfg(target_os = "macos")]
mod macos_observer;
#[cfg(target_os = "macos")]
mod macos_overlay_panel;
#[cfg(target_os = "macos")]
mod macos_restore;
#[cfg(target_os = "macos")]
mod macos_selection;
#[cfg(target_os = "macos")]
mod macos_text_marker;

pub use clipboard::SystemClipboard;
pub use note_result::NoteResultPayload;
#[cfg(target_os = "macos")]
pub use overlay::install_mouse_dismiss_monitor;
pub use overlay::TauriOverlay;
pub(crate) use overlay_readiness::OverlaySurface;
pub(crate) use overlay_window::is_current_caller;

#[cfg(target_os = "macos")]
pub use macos_accessibility::MacAccessibility;

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
