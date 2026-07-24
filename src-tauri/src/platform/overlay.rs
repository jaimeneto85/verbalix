use crate::{
    application::OverlayPort,
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

    pub fn surface_ready(&self, label: &str) -> Result<(), VerbalixError> {
        self.dispatcher
            .dispatch(OverlayCommand::SurfaceReady(OverlaySurface::from_label(
                label,
            )?))
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

    fn show_result(&self, bounds: Rect, payload: NoteResultPayload) -> Result<(), VerbalixError> {
        self.note_result.publish(payload.clone())?;
        self.dispatcher
            .dispatch(OverlayCommand::ShowResult(bounds, payload))
    }
}

impl OverlayPort for TauriOverlay {
    fn show_toolbar(&self, bounds: Rect) -> Result<(), VerbalixError> {
        self.dispatcher
            .dispatch(OverlayCommand::ShowToolbar(bounds))
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

    fn hide_all(&self) -> Result<(), VerbalixError> {
        self.dispatcher.dispatch(OverlayCommand::HideAll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingDispatcher {
        commands: Mutex<Vec<OverlayCommand>>,
        fail: bool,
    }

    impl OverlayDispatcher for RecordingDispatcher {
        fn dispatch(&self, command: OverlayCommand) -> Result<(), VerbalixError> {
            if self.fail {
                return Err(VerbalixError::LocalFailure);
            }
            self.commands.lock().unwrap().push(command);
            Ok(())
        }
    }

    fn bounds() -> Rect {
        Rect {
            x: 200.0,
            y: 300.0,
            width: 100.0,
            height: 20.0,
        }
    }

    #[test]
    fn worker_thread_forwards_every_window_operation_to_dispatcher() {
        let dispatcher = Arc::new(RecordingDispatcher::default());
        let overlay = Arc::new(TauriOverlay::with_dispatcher(dispatcher.clone()));
        let worker = std::thread::spawn(move || {
            overlay.show_toolbar(bounds()).unwrap();
            overlay
                .show_error(bounds(), "Entre no Verbalix para continuar.")
                .unwrap();
            overlay.show_note(bounds(), "translated").unwrap();
            overlay
                .show_preview(bounds(), uuid::Uuid::new_v4(), "preview")
                .unwrap();
            overlay.show_undo(bounds(), "applied").unwrap();
            overlay.surface_ready("toolbar").unwrap();
            overlay.hide_all().unwrap();
        });

        worker.join().unwrap();

        let commands = dispatcher.commands.lock().unwrap();
        assert!(matches!(commands[0], OverlayCommand::ShowToolbar(_)));
        assert!(matches!(commands[1], OverlayCommand::ShowResult(_, _)));
        assert!(matches!(commands[2], OverlayCommand::ShowResult(_, _)));
        assert!(matches!(commands[3], OverlayCommand::ShowResult(_, _)));
        assert!(matches!(commands[4], OverlayCommand::ShowResult(_, _)));
        assert_eq!(
            commands[5],
            OverlayCommand::SurfaceReady(OverlaySurface::Toolbar)
        );
        assert_eq!(commands[6], OverlayCommand::HideAll);
    }

    #[test]
    fn dispatch_failure_is_returned_without_panicking() {
        let overlay = TauriOverlay::with_dispatcher(Arc::new(RecordingDispatcher {
            commands: Mutex::new(Vec::new()),
            fail: true,
        }));

        assert!(matches!(
            overlay.show_toolbar(bounds()),
            Err(VerbalixError::LocalFailure)
        ));
        assert!(matches!(
            overlay.hide_all(),
            Err(VerbalixError::LocalFailure)
        ));
        assert!(matches!(
            overlay.surface_ready("main"),
            Err(VerbalixError::LocalFailure)
        ));
    }
}
