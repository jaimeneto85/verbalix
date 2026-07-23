mod clipboard;
mod note_result;
mod overlay;
mod overlay_dispatcher;

#[cfg(target_os = "macos")]
mod macos_accessibility;
#[cfg(target_os = "macos")]
mod macos_geometry;
#[cfg(target_os = "macos")]
mod macos_observer;
#[cfg(target_os = "macos")]
mod macos_restore;

pub use clipboard::SystemClipboard;
pub use note_result::NoteResultPayload;
#[cfg(target_os = "macos")]
pub use overlay::install_mouse_dismiss_monitor;
pub use overlay::TauriOverlay;

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
