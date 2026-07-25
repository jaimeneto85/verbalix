use crate::{
    application::{MutationProjection, MutationReceipt, PublicationGuard},
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
        lease: &PublicationGuard,
    ) -> Result<MutationReceipt, VerbalixError> {
        if !lease.try_claim_write() {
            return Err(VerbalixError::StaleSelection);
        }
        self.replace(expected, text)?;
        Ok(MutationReceipt {
            id: uuid::Uuid::new_v4(),
            snapshot_id: expected.id,
            request_id: lease.request_id(),
        })
    }
    fn replace_guarded_with_id(
        &self,
        expected: &SelectionSnapshot,
        text: &str,
        lease: &PublicationGuard,
        _mutation_id: uuid::Uuid,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.replace_guarded(expected, text, lease)
    }
    fn restore_guarded(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
        lease: &PublicationGuard,
    ) -> Result<MutationReceipt, VerbalixError> {
        if !lease.try_claim_write() {
            return Err(VerbalixError::StaleSelection);
        }
        self.restore(expected, transformed_text)?;
        Ok(MutationReceipt {
            id: uuid::Uuid::new_v4(),
            snapshot_id: expected.id,
            request_id: lease.request_id(),
        })
    }
    fn restore_guarded_with_id(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
        lease: &PublicationGuard,
        _mutation_id: uuid::Uuid,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.restore_guarded(expected, transformed_text, lease)
    }
    fn restore(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
    ) -> Result<(), VerbalixError>;

    fn discard_snapshot(&self, _snapshot_id: uuid::Uuid) {}

    fn reconcile_mutation(
        &self,
        _mutation_id: uuid::Uuid,
    ) -> Result<Option<MutationProjection>, VerbalixError> {
        Ok(None)
    }
}

pub trait OverlayPort: Send + Sync {
    fn show_toolbar(&self, bounds: Rect) -> Result<(), VerbalixError>;
    fn show_toolbar_guarded(
        &self,
        bounds: Rect,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        if guard.may_publish() {
            self.show_toolbar(bounds)
        } else {
            Ok(())
        }
    }
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
        if guard.may_publish() {
            self.show_note(bounds, text)
        } else {
            Ok(())
        }
    }
    fn show_preview_guarded(
        &self,
        bounds: Rect,
        request_id: uuid::Uuid,
        text: &str,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        if guard.may_publish() {
            self.show_preview(bounds, request_id, text)
        } else {
            Ok(())
        }
    }
    fn show_undo_guarded(
        &self,
        bounds: Rect,
        text: &str,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        if guard.may_publish() {
            self.show_undo(bounds, text)
        } else {
            Ok(())
        }
    }
    fn hide_all(&self) -> Result<(), VerbalixError>;
}

pub trait ClipboardPort: Send + Sync {
    fn read_text(&self) -> Result<Option<String>, VerbalixError>;
}
