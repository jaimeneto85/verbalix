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

#[derive(Debug, Default, PartialEq)]
struct ExecutionTrace {
    events: Vec<&'static str>,
    result_emits: usize,
    visible: bool,
}

fn execute_commands(dispatcher: &SequenceDispatcher) -> ExecutionTrace {
    dispatcher.commands.lock().unwrap().iter().fold(
        ExecutionTrace::default(),
        |mut trace, command| {
            match command {
                OverlayCommand::ShowToolbar(_, permit) => {
                    assert!(execute_if_publishable(
                        permit.as_ref(),
                        || Ok(()),
                        || Ok(()),
                        || Ok(()),
                    )
                    .unwrap());
                    trace.events.push("toolbar");
                    trace.visible = true;
                }
                OverlayCommand::ShowResult(_, payload, permit) => {
                    assert!(execute_if_publishable(
                        permit.as_ref(),
                        || Ok(()),
                        || Ok(()),
                        || Ok(()),
                    )
                    .unwrap());
                    trace.events.push(match payload.mode {
                        NoteMode::Preview => "preview",
                        NoteMode::Undo => "undo",
                        NoteMode::Error => "error",
                        NoteMode::Result => "result",
                    });
                    trace.result_emits += 1;
                    trace.visible = true;
                }
                OverlayCommand::HideAll => {
                    trace.events.push("hide");
                    trace.visible = false;
                }
            }
            trace
        },
    )
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

    assert_eq!(
        execute_commands(&dispatcher),
        ExecutionTrace {
            events: vec!["preview", "error"],
            result_emits: 2,
            visible: true,
        }
    );
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

    assert_eq!(
        execute_commands(&dispatcher),
        ExecutionTrace {
            events: vec!["undo", "error"],
            result_emits: 2,
            visible: true,
        }
    );
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

    assert_eq!(
        execute_commands(&dispatcher),
        ExecutionTrace {
            events: vec!["toolbar", "error"],
            result_emits: 1,
            visible: true,
        }
    );
    assert_eq!(
        overlay.current_note_result().unwrap().unwrap().mode,
        NoteMode::Error
    );
}
