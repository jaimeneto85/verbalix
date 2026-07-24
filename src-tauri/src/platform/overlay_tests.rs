use super::*;
use crate::{application::TransformLease, platform::overlay_publication::execute_if_publishable};
use std::sync::Mutex;

#[derive(Default)]
struct RecordingDispatcher {
    commands: Mutex<Vec<OverlayCommand>>,
    ready: Mutex<Vec<(OverlaySurface, uuid::Uuid)>>,
    fail: bool,
}

#[async_trait::async_trait]
impl OverlayDispatcher for RecordingDispatcher {
    fn dispatch(&self, command: OverlayCommand) -> Result<(), VerbalixError> {
        if self.fail {
            return Err(VerbalixError::LocalFailure);
        }
        self.commands.lock().unwrap().push(command);
        Ok(())
    }

    async fn surface_ready(
        &self,
        surface: OverlaySurface,
        generation: uuid::Uuid,
    ) -> Result<bool, VerbalixError> {
        if self.fail {
            return Err(VerbalixError::LocalFailure);
        }
        self.ready.lock().unwrap().push((surface, generation));
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
        overlay.hide_all().unwrap();
    });

    worker.join().unwrap();

    let commands = dispatcher.commands.lock().unwrap();
    assert!(matches!(commands[0], OverlayCommand::ShowToolbar(_, _)));
    assert!(matches!(commands[1], OverlayCommand::ShowResult(_, _, _)));
    assert!(matches!(commands[2], OverlayCommand::ShowResult(_, _, _)));
    assert!(matches!(commands[3], OverlayCommand::ShowResult(_, _, _)));
    assert!(matches!(commands[4], OverlayCommand::ShowResult(_, _, _)));
    assert_eq!(commands[5], OverlayCommand::HideAll);
}

#[test]
fn dispatch_failure_is_returned_without_panicking() {
    let overlay = TauriOverlay::with_dispatcher(Arc::new(RecordingDispatcher {
        commands: Mutex::new(Vec::new()),
        ready: Mutex::new(Vec::new()),
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
}

#[tokio::test]
async fn readiness_returns_an_ack_and_rejects_unknown_surfaces() {
    let dispatcher = Arc::new(RecordingDispatcher::default());
    let overlay = TauriOverlay::with_dispatcher(dispatcher.clone());
    let generation = uuid::Uuid::new_v4();

    assert!(overlay.surface_ready("toolbar", generation).await.unwrap());
    assert_eq!(
        dispatcher.ready.lock().unwrap().as_slice(),
        &[(OverlaySurface::Toolbar, generation)]
    );
    assert!(matches!(
        overlay.surface_ready("main", generation).await,
        Err(VerbalixError::LocalFailure)
    ));
}

#[test]
fn queued_result_cancelled_before_execution_has_no_effect_or_current_payload() {
    let dispatcher = Arc::new(RecordingDispatcher::default());
    let overlay = TauriOverlay::with_dispatcher(dispatcher.clone());
    let guard = Arc::new(TransformLease::new(
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
    ));
    overlay
        .show_error_guarded(bounds(), "stale failure", guard.clone())
        .unwrap();
    assert!(overlay.current_note_result().unwrap().is_some());

    guard.cancel();
    let queued_guard = {
        let commands = dispatcher.commands.lock().unwrap();
        match &commands[0] {
            OverlayCommand::ShowResult(_, _, guard) => guard.clone(),
            _ => panic!("expected a queued result"),
        }
    };
    let executions = std::cell::Cell::new(0);
    let executed = execute_if_publishable(
        queued_guard.as_ref(),
        || {
            executions.set(executions.get() + 1);
            Ok(())
        },
        || Ok(()),
        || Ok(()),
    )
    .unwrap();

    assert!(!executed);
    assert_eq!(executions.get(), 0);
    assert_eq!(overlay.current_note_result().unwrap(), None);
}

#[test]
fn queued_toolbar_cancelled_before_execution_has_no_effect() {
    let dispatcher = Arc::new(RecordingDispatcher::default());
    let overlay = TauriOverlay::with_dispatcher(dispatcher.clone());
    let guard = Arc::new(TransformLease::new(uuid::Uuid::new_v4(), uuid::Uuid::nil()));
    overlay
        .show_toolbar_guarded(bounds(), guard.clone())
        .unwrap();
    guard.cancel();
    let queued_guard = {
        let commands = dispatcher.commands.lock().unwrap();
        match &commands[0] {
            OverlayCommand::ShowToolbar(_, guard) => guard.clone(),
            _ => panic!("expected a queued toolbar"),
        }
    };
    let executions = std::cell::Cell::new(0);
    let executed = execute_if_publishable(
        queued_guard.as_ref(),
        || {
            executions.set(executions.get() + 1);
            Ok(())
        },
        || Ok(()),
        || Ok(()),
    )
    .unwrap();

    assert!(!executed);
    assert_eq!(executions.get(), 0);
}
