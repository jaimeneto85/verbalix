use super::*;
use crate::{
    application::TransformLease,
    platform::{
        note_result::NoteMode, overlay_publication::execute_if_publishable,
        overlay_readiness::OverlaySurface,
    },
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Default)]
struct SequenceDispatcher {
    commands: Mutex<Vec<OverlayCommand>>,
}

#[async_trait::async_trait]
impl OverlayDispatcher for SequenceDispatcher {
    fn dispatch(&self, command: OverlayCommand) -> Result<(), VerbalixError> {
        self.commands.lock().unwrap().push(command);
        Ok(())
    }

    async fn surface_ready(
        &self,
        _surface: OverlaySurface,
        _generation: Uuid,
    ) -> Result<bool, VerbalixError> {
        Ok(true)
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

fn execute_commands(dispatcher: &SequenceDispatcher) -> Vec<&'static str> {
    dispatcher
        .commands
        .lock()
        .unwrap()
        .iter()
        .map(|command| match command {
            OverlayCommand::ShowToolbar(_, permit) => {
                assert!(
                    execute_if_publishable(permit.as_ref(), || Ok(()), || Ok(()), || Ok(()),)
                        .unwrap()
                );
                "toolbar"
            }
            OverlayCommand::ShowResult(_, payload, permit) => {
                assert!(
                    execute_if_publishable(permit.as_ref(), || Ok(()), || Ok(()), || Ok(()),)
                        .unwrap()
                );
                match payload.mode {
                    NoteMode::Preview => "preview",
                    NoteMode::Undo => "undo",
                    NoteMode::Error => "error",
                    NoteMode::Result => "result",
                }
            }
            OverlayCommand::HideAll => "hide",
        })
        .collect()
}

fn guarded_overlay() -> (TauriOverlay, Arc<SequenceDispatcher>, Arc<TransformLease>) {
    let dispatcher = Arc::new(SequenceDispatcher::default());
    let overlay = TauriOverlay::with_dispatcher(dispatcher.clone());
    let guard = Arc::new(TransformLease::new(Uuid::new_v4(), Uuid::new_v4()));
    (overlay, dispatcher, guard)
}

#[test]
fn preview_then_apply_failure_publishes_the_guarded_error() {
    let (overlay, dispatcher, guard) = guarded_overlay();
    overlay
        .show_preview_guarded(bounds(), Uuid::new_v4(), "preview", guard.clone())
        .unwrap();
    overlay
        .show_error_guarded(bounds(), "apply failed", guard)
        .unwrap();

    assert_eq!(execute_commands(&dispatcher), ["preview", "error"]);
    assert_eq!(
        overlay.current_note_result().unwrap().unwrap().mode,
        NoteMode::Error
    );
}

#[test]
fn undo_then_restore_failure_publishes_the_guarded_error() {
    let (overlay, dispatcher, guard) = guarded_overlay();
    overlay
        .show_undo_guarded(bounds(), "transformed", guard.clone())
        .unwrap();
    overlay
        .show_error_guarded(bounds(), "undo failed", guard)
        .unwrap();

    assert_eq!(execute_commands(&dispatcher), ["undo", "error"]);
    assert_eq!(
        overlay.current_note_result().unwrap().unwrap().mode,
        NoteMode::Error
    );
}

#[test]
fn toolbar_then_pin_failure_publishes_the_guarded_error() {
    let (overlay, dispatcher, guard) = guarded_overlay();
    overlay
        .show_toolbar_guarded(bounds(), guard.clone())
        .unwrap();
    overlay
        .show_error_guarded(bounds(), "pin failed", guard)
        .unwrap();

    assert_eq!(execute_commands(&dispatcher), ["toolbar", "error"]);
    assert_eq!(
        overlay.current_note_result().unwrap().unwrap().mode,
        NoteMode::Error
    );
}
