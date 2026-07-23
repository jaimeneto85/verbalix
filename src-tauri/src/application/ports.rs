use crate::domain::{Rect, SelectionSnapshot, VerbalixError};

pub trait SelectionPort: Send + Sync {
    fn permission_granted(&self, prompt: bool) -> bool;
    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError>;
    fn replace(&self, expected: &SelectionSnapshot, text: &str) -> Result<(), VerbalixError>;
}

pub trait OverlayPort: Send + Sync {
    fn show_toolbar(&self, bounds: Rect) -> Result<(), VerbalixError>;
    fn show_note(&self, bounds: Rect, text: &str) -> Result<(), VerbalixError>;
    fn hide_all(&self) -> Result<(), VerbalixError>;
}

pub trait ClipboardPort: Send + Sync {
    fn read_text(&self) -> Result<Option<String>, VerbalixError>;
    fn write_text(&self, text: &str) -> Result<(), VerbalixError>;
}
