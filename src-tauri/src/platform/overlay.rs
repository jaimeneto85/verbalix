use crate::{
    application::{OverlayPort, PublicationGuard},
    domain::{Rect, VerbalixError},
    platform::{
        note_result::{NoteMode, NoteResultPayload, NoteResultState},
        overlay_dispatcher::{MainThreadOverlayDispatcher, OverlayCommand, OverlayDispatcher},
        overlay_readiness::OverlaySurface,
    },
};
use std::sync::Arc;
use tauri::AppHandle;

#[cfg(target_os = "macos")]
pub fn install_mouse_dismiss_monitor(callback: std::sync::Arc<dyn Fn() + Send + Sync>) {
    use block2::RcBlock;
    use objc2_app_kit::{NSEvent, NSEventMask};
    let block = RcBlock::new(move |_event| callback());
    if let Some(monitor) = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
        NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown,
        &block,
    ) {
        std::mem::forget(monitor);
    }
}

pub struct TauriOverlay {
    dispatcher: Arc<dyn OverlayDispatcher>,
    note_result: Arc<NoteResultState>,
}

impl TauriOverlay {
    pub fn new(app: AppHandle) -> Self {
        Self {
            dispatcher: Arc::new(MainThreadOverlayDispatcher::new(app)),
            note_result: Arc::new(NoteResultState::default()),
        }
    }

    #[cfg(test)]
    fn with_dispatcher(dispatcher: Arc<dyn OverlayDispatcher>) -> Self {
        Self {
            dispatcher,
            note_result: Arc::new(NoteResultState::default()),
        }
    }

    pub fn current_note_result(&self) -> Result<Option<NoteResultPayload>, VerbalixError> {
        self.note_result.current()
    }

    pub async fn surface_ready(
        &self,
        label: &str,
        generation: uuid::Uuid,
    ) -> Result<bool, VerbalixError> {
        self.dispatcher
            .surface_ready(OverlaySurface::from_label(label)?, generation)
            .await
    }

    pub fn show_error(&self, bounds: Rect, message: &str) -> Result<(), VerbalixError> {
        self.show_result(
            bounds,
            NoteResultPayload {
                mode: NoteMode::Error,
                request_id: None,
                text: message.to_owned(),
            },
        )
    }

    pub(crate) fn show_error_guarded(
        &self,
        bounds: Rect,
        message: &str,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        self.show_result_guarded(
            bounds,
            NoteResultPayload {
                mode: NoteMode::Error,
                request_id: None,
                text: message.to_owned(),
            },
            Some(guard),
        )
    }

    fn show_result(&self, bounds: Rect, payload: NoteResultPayload) -> Result<(), VerbalixError> {
        self.show_result_guarded(bounds, payload, None)
    }

    fn show_result_guarded(
        &self,
        bounds: Rect,
        payload: NoteResultPayload,
        guard: Option<PublicationGuard>,
    ) -> Result<(), VerbalixError> {
        if guard.as_ref().is_some_and(|guard| !guard.may_publish()) {
            return Ok(());
        }
        self.note_result.publish(payload.clone(), guard.clone())?;
        self.dispatcher
            .dispatch(OverlayCommand::ShowResult(bounds, payload, guard))
    }
}

impl OverlayPort for TauriOverlay {
    fn show_toolbar(&self, bounds: Rect) -> Result<(), VerbalixError> {
        self.dispatcher
            .dispatch(OverlayCommand::ShowToolbar(bounds, None))
    }

    fn show_toolbar_guarded(
        &self,
        bounds: Rect,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        self.dispatcher
            .dispatch(OverlayCommand::ShowToolbar(bounds, Some(guard)))
    }

    fn show_note(&self, bounds: Rect, text: &str) -> Result<(), VerbalixError> {
        self.show_result(
            bounds,
            NoteResultPayload {
                mode: NoteMode::Result,
                request_id: None,
                text: text.to_owned(),
            },
        )
    }

    fn show_preview(
        &self,
        bounds: Rect,
        request_id: uuid::Uuid,
        text: &str,
    ) -> Result<(), VerbalixError> {
        self.show_result(
            bounds,
            NoteResultPayload {
                mode: NoteMode::Preview,
                request_id: Some(request_id),
                text: text.to_owned(),
            },
        )
    }

    fn show_undo(&self, bounds: Rect, text: &str) -> Result<(), VerbalixError> {
        self.show_result(
            bounds,
            NoteResultPayload {
                mode: NoteMode::Undo,
                request_id: None,
                text: text.to_owned(),
            },
        )
    }

    fn show_note_guarded(
        &self,
        bounds: Rect,
        text: &str,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        self.show_result_guarded(
            bounds,
            NoteResultPayload {
                mode: NoteMode::Result,
                request_id: None,
                text: text.to_owned(),
            },
            Some(guard),
        )
    }

    fn show_preview_guarded(
        &self,
        bounds: Rect,
        request_id: uuid::Uuid,
        text: &str,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        self.show_result_guarded(
            bounds,
            NoteResultPayload {
                mode: NoteMode::Preview,
                request_id: Some(request_id),
                text: text.to_owned(),
            },
            Some(guard),
        )
    }

    fn show_undo_guarded(
        &self,
        bounds: Rect,
        text: &str,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        self.show_result_guarded(
            bounds,
            NoteResultPayload {
                mode: NoteMode::Undo,
                request_id: None,
                text: text.to_owned(),
            },
            Some(guard),
        )
    }

    fn hide_all(&self) -> Result<(), VerbalixError> {
        self.dispatcher.dispatch(OverlayCommand::HideAll)
    }
}

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
