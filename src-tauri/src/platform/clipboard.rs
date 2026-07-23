use crate::{application::ClipboardPort, domain::VerbalixError};
use std::sync::Mutex;
use std::{thread, time::Duration};

pub struct SystemClipboard {
    clipboard: Mutex<arboard::Clipboard>,
}

impl SystemClipboard {
    pub fn new() -> Result<Self, VerbalixError> {
        Ok(Self {
            clipboard: Mutex::new(
                arboard::Clipboard::new().map_err(|_| VerbalixError::LocalFailure)?,
            ),
        })
    }

    #[cfg(target_os = "macos")]
    pub fn copy_selection_preserving_clipboard(&self) -> Result<String, VerbalixError> {
        let previous = self.read_text()?;
        {
            let mut clipboard = self
                .clipboard
                .lock()
                .map_err(|_| VerbalixError::LocalFailure)?;
            clipboard.clear().map_err(|_| VerbalixError::LocalFailure)?;
        }
        post_copy_shortcut()?;
        thread::sleep(Duration::from_millis(120));
        let selected = self
            .read_text()?
            .filter(|value| !value.trim().is_empty())
            .ok_or(VerbalixError::SelectionUnavailable)?;
        match previous {
            Some(value) => self.write_text(&value)?,
            None => {
                self.clipboard
                    .lock()
                    .map_err(|_| VerbalixError::LocalFailure)?
                    .clear()
                    .map_err(|_| VerbalixError::LocalFailure)?;
            }
        }
        Ok(selected)
    }
}

impl ClipboardPort for SystemClipboard {
    fn read_text(&self) -> Result<Option<String>, VerbalixError> {
        match self
            .clipboard
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)?
            .get_text()
        {
            Ok(value) => Ok(Some(value)),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(_) => Err(VerbalixError::LocalFailure),
        }
    }

    fn write_text(&self, text: &str) -> Result<(), VerbalixError> {
        self.clipboard
            .lock()
            .map_err(|_| VerbalixError::LocalFailure)?
            .set_text(text.to_owned())
            .map_err(|_| VerbalixError::LocalFailure)
    }
}

#[cfg(target_os = "macos")]
fn post_copy_shortcut() -> Result<(), VerbalixError> {
    use core_foundation::base::{CFRelease, CFTypeRef};
    use std::ffi::c_void;

    type CGEventRef = *const c_void;
    type CGEventSourceRef = *const c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn CGEventSourceCreate(state_id: u32) -> CGEventSourceRef;
        fn CGEventCreateKeyboardEvent(
            source: CGEventSourceRef,
            virtual_key: u16,
            key_down: bool,
        ) -> CGEventRef;
        fn CGEventSetFlags(event: CGEventRef, flags: u64);
        fn CGEventPost(tap: u32, event: CGEventRef);
    }

    let source = unsafe { CGEventSourceCreate(1) };
    if source.is_null() {
        return Err(VerbalixError::LocalFailure);
    }
    for key_down in [true, false] {
        let event = unsafe { CGEventCreateKeyboardEvent(source, 8, key_down) };
        if event.is_null() {
            unsafe { CFRelease(source as CFTypeRef) };
            return Err(VerbalixError::LocalFailure);
        }
        unsafe {
            CGEventSetFlags(event, 1 << 20);
            CGEventPost(0, event);
            CFRelease(event as CFTypeRef);
        }
    }
    unsafe { CFRelease(source as CFTypeRef) };
    Ok(())
}
