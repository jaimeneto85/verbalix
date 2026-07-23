use crate::{application::ClipboardPort, domain::VerbalixError};
use std::sync::Mutex;

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
