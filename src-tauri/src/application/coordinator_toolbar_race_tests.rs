use super::*;
use crate::application::PublicationGuard;
use std::{
    sync::{Arc, Condvar},
    thread,
};

#[derive(Default)]
struct ToolbarGate {
    entered: bool,
    released: bool,
}

#[derive(Default)]
struct BlockingToolbarOverlay {
    gate: (Mutex<ToolbarGate>, Condvar),
    toolbar_publications: AtomicUsize,
    hides: AtomicUsize,
}

impl BlockingToolbarOverlay {
    fn wait_until_entered(&self) {
        let (state, changed) = &self.gate;
        let mut state = state.lock().unwrap();
        while !state.entered {
            state = changed.wait(state).unwrap();
        }
    }

    fn release(&self) {
        let (state, changed) = &self.gate;
        let mut state = state.lock().unwrap();
        state.released = true;
        changed.notify_all();
    }
}

impl OverlayPort for BlockingToolbarOverlay {
    fn show_toolbar(&self, _bounds: Rect) -> Result<(), VerbalixError> {
        panic!("coordinator must use show_toolbar_guarded")
    }

    fn show_toolbar_guarded(
        &self,
        _bounds: Rect,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        let (state, changed) = &self.gate;
        let mut state = state.lock().unwrap();
        state.entered = true;
        changed.notify_all();
        while !state.released {
            state = changed.wait(state).unwrap();
        }
        drop(state);
        if guard.may_publish() {
            self.toolbar_publications.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }

    fn show_note(&self, _bounds: Rect, _text: &str) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn show_preview(
        &self,
        _bounds: Rect,
        _request_id: Uuid,
        _text: &str,
    ) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn show_undo(&self, _bounds: Rect, _text: &str) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn hide_all(&self) -> Result<(), VerbalixError> {
        self.hides.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn blocked_toolbar() -> (
    Arc<SelectionCoordinator>,
    Arc<BlockingToolbarOverlay>,
    SelectionSnapshot,
) {
    let captured = snapshot(true);
    let selection = Arc::new(FakeSelection {
        snapshot: Mutex::new(captured.clone()),
        replacements: Mutex::new(Vec::new()),
        fail_write: false,
    });
    let overlay = Arc::new(BlockingToolbarOverlay::default());
    let coordinator = Arc::new(SelectionCoordinator::new(
        selection,
        overlay.clone(),
        Arc::new(FakeProvider),
    ));
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(captured.clone())))
        .unwrap();
    (coordinator, overlay, captured)
}

fn spawn_debounce(
    coordinator: Arc<SelectionCoordinator>,
    id: Uuid,
) -> thread::JoinHandle<Result<(), VerbalixError>> {
    thread::spawn(move || coordinator.dispatch(SelectionEvent::DebounceElapsed(id)))
}

#[test]
fn candidate_progresses_while_toolbar_is_blocked_and_cancels_a_publication() {
    let (coordinator, overlay, captured) = blocked_toolbar();
    let running = spawn_debounce(coordinator.clone(), captured.id);
    overlay.wait_until_entered();

    assert!(coordinator.state.try_lock().is_ok());
    assert!(coordinator.presentation.try_lock().is_ok());
    let next = snapshot(true);
    let next_id = next.id;
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(next)))
        .unwrap();
    overlay.release();
    running.join().unwrap().unwrap();

    assert_eq!(overlay.toolbar_publications.load(Ordering::SeqCst), 0);
    assert!(matches!(
        &*coordinator.state.lock().unwrap(),
        SelectionState::Candidate(snapshot) if snapshot.id == next_id
    ));
}

#[test]
fn invalidation_progresses_while_toolbar_is_blocked_and_cancels_a_publication() {
    let (coordinator, overlay, captured) = blocked_toolbar();
    let running = spawn_debounce(coordinator.clone(), captured.id);
    overlay.wait_until_entered();

    assert!(coordinator.state.try_lock().is_ok());
    assert!(coordinator.presentation.try_lock().is_ok());
    coordinator.dispatch(SelectionEvent::Invalidated).unwrap();
    overlay.release();
    running.join().unwrap().unwrap();

    assert_eq!(overlay.toolbar_publications.load(Ordering::SeqCst), 0);
    assert_eq!(overlay.hides.load(Ordering::SeqCst), 1);
    assert!(matches!(
        &*coordinator.state.lock().unwrap(),
        SelectionState::Idle
    ));
}
