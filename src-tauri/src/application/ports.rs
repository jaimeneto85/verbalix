use crate::{
    application::{PublicationGuard, TransformLease},
    domain::{Rect, SelectionSnapshot, VerbalixError},
};

pub trait SelectionPort: Send + Sync {
    fn permission_granted(&self, prompt: bool) -> bool;
    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError>;
    fn replace(&self, expected: &SelectionSnapshot, text: &str) -> Result<(), VerbalixError>;
    fn replace_guarded(
        &self,
        expected: &SelectionSnapshot,
        text: &str,
        lease: &TransformLease,
    ) -> Result<(), VerbalixError> {
        if !lease.try_claim_write() {
            return Err(VerbalixError::StaleSelection);
        }
        self.replace(expected, text)
    }
    fn restore(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
    ) -> Result<(), VerbalixError>;
}

pub trait OverlayPort: Send + Sync {
    fn show_toolbar(&self, bounds: Rect) -> Result<(), VerbalixError>;
    fn show_note(&self, bounds: Rect, text: &str) -> Result<(), VerbalixError>;
    fn show_preview(
        &self,
        bounds: Rect,
        request_id: uuid::Uuid,
        text: &str,
    ) -> Result<(), VerbalixError>;
    fn show_undo(&self, bounds: Rect, text: &str) -> Result<(), VerbalixError>;
    fn show_note_guarded(
        &self,
        bounds: Rect,
        text: &str,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        guard
            .may_publish()
            .then(|| self.show_note(bounds, text))
            .unwrap_or(Ok(()))
    }
    fn show_preview_guarded(
        &self,
        bounds: Rect,
        request_id: uuid::Uuid,
        text: &str,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        guard
            .may_publish()
            .then(|| self.show_preview(bounds, request_id, text))
            .unwrap_or(Ok(()))
    }
    fn show_undo_guarded(
        &self,
        bounds: Rect,
        text: &str,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        guard
            .may_publish()
            .then(|| self.show_undo(bounds, text))
            .unwrap_or(Ok(()))
    }
    fn hide_all(&self) -> Result<(), VerbalixError>;
}

pub trait ClipboardPort: Send + Sync {
    fn read_text(&self) -> Result<Option<String>, VerbalixError>;
}
