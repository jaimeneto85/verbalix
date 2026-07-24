use super::*;
use crate::application::TransformLease;
use std::{
    sync::{Arc, Condvar},
    thread,
};

#[derive(Clone, Copy)]
enum RestoreGatePoint {
    BeforeClaim,
    AfterClaim,
}

#[derive(Default)]
struct RestoreGate {
    entered: bool,
    released: bool,
}

struct BlockingRestoreSelection {
    current: Mutex<SelectionSnapshot>,
    restores: Mutex<Vec<String>>,
    gate: (Mutex<RestoreGate>, Condvar),
    point: RestoreGatePoint,
}

impl BlockingRestoreSelection {
    fn wait_until_entered(&self) {
        let (state, changed) = &self.gate;
        let mut state = state.lock().unwrap();
        while !state.entered {
            state = changed.wait(state).unwrap();
        }
    }

    fn enter_and_wait(&self) {
        let (state, changed) = &self.gate;
        let mut state = state.lock().unwrap();
        state.entered = true;
        changed.notify_all();
        while !state.released {
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

impl SelectionPort for BlockingRestoreSelection {
    fn permission_granted(&self, _prompt: bool) -> bool {
        true
    }

    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        Ok(self.current.lock().unwrap().clone())
    }

    fn replace(&self, _expected: &SelectionSnapshot, text: &str) -> Result<(), VerbalixError> {
        self.current.lock().unwrap().text = text.to_owned();
        Ok(())
    }

    fn restore_guarded(
        &self,
        _expected: &SelectionSnapshot,
        transformed_text: &str,
        lease: &TransformLease,
    ) -> Result<(), VerbalixError> {
        if matches!(self.point, RestoreGatePoint::BeforeClaim) {
            self.enter_and_wait();
        }
        if !lease.try_claim_write() {
            return Err(VerbalixError::StaleSelection);
        }
        if matches!(self.point, RestoreGatePoint::AfterClaim) {
            self.enter_and_wait();
        }
        self.restores
            .lock()
            .unwrap()
            .push(transformed_text.to_owned());
        Ok(())
    }

    fn restore(
        &self,
        _expected: &SelectionSnapshot,
        _transformed_text: &str,
    ) -> Result<(), VerbalixError> {
        panic!("coordinator must use restore_guarded")
    }
}

fn ready_for_undo(
    point: RestoreGatePoint,
) -> (
    Arc<SelectionCoordinator>,
    Arc<BlockingRestoreSelection>,
    Arc<EventOverlay>,
) {
    let captured = snapshot(42, "editor-a");
    let selection = Arc::new(BlockingRestoreSelection {
        current: Mutex::new(captured.clone()),
        restores: Mutex::new(Vec::new()),
        gate: (Mutex::new(RestoreGate::default()), Condvar::new()),
        point,
    });
    let overlay = Arc::new(EventOverlay::default());
    let coordinator = Arc::new(SelectionCoordinator::new(
        selection.clone(),
        overlay.clone(),
        Arc::new(ImmediateProvider::default()),
    ));
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(captured.clone())))
        .unwrap();
    coordinator
        .dispatch(SelectionEvent::DebounceElapsed(captured.id))
        .unwrap();
    let input = request();
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(coordinator.transform(captured.id, input, "token", false))
        .unwrap();
    (coordinator, selection, overlay)
}

fn supersede_with_visible_b(coordinator: &SelectionCoordinator) -> Uuid {
    let next = snapshot(84, "editor-b");
    let next_id = next.id;
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(next)))
        .unwrap();
    coordinator
        .dispatch(SelectionEvent::DebounceElapsed(next_id))
        .unwrap();
    next_id
}

fn assert_b_survives(coordinator: &SelectionCoordinator, overlay: &EventOverlay, next_id: Uuid) {
    assert!(matches!(
        &*coordinator.state.lock().unwrap(),
        SelectionState::ToolbarVisible(snapshot) if snapshot.id == next_id
    ));
    assert_eq!(
        overlay
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| **event == "hide")
            .count(),
        1
    );
}

#[test]
fn candidate_before_restore_claim_cancels_restore_and_keeps_b_visible() {
    let (coordinator, selection, overlay) = ready_for_undo(RestoreGatePoint::BeforeClaim);
    let running = {
        let coordinator = coordinator.clone();
        thread::spawn(move || coordinator.undo("translated"))
    };
    selection.wait_until_entered();
    assert!(coordinator.state.try_lock().is_ok());
    assert!(coordinator.active_transform.try_lock().is_ok());
    let next_id = supersede_with_visible_b(&coordinator);
    selection.release();

    assert!(matches!(
        running.join().unwrap(),
        Err(VerbalixError::StaleSelection)
    ));
    assert!(selection.restores.lock().unwrap().is_empty());
    assert_b_survives(&coordinator, &overlay, next_id);
}

#[test]
fn candidate_after_restore_claim_allows_one_restore_without_hiding_b() {
    let (coordinator, selection, overlay) = ready_for_undo(RestoreGatePoint::AfterClaim);
    let running = {
        let coordinator = coordinator.clone();
        thread::spawn(move || coordinator.undo("translated"))
    };
    selection.wait_until_entered();
    assert!(coordinator.state.try_lock().is_ok());
    assert!(coordinator.active_transform.try_lock().is_ok());
    let next_id = supersede_with_visible_b(&coordinator);
    selection.release();

    running.join().unwrap().unwrap();
    assert_eq!(
        selection.restores.lock().unwrap().as_slice(),
        ["translated"]
    );
    assert_b_survives(&coordinator, &overlay, next_id);
}
