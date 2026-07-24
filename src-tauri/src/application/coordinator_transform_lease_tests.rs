use super::*;
use crate::application::TransformLease;
use std::{
    sync::{Arc, Condvar},
    thread,
};

#[derive(Clone, Copy)]
enum GatePoint {
    BeforeClaim,
    AfterClaim,
}

#[derive(Default)]
struct GateState {
    entered: bool,
    released: bool,
}

struct BlockingClaimSelection {
    current: Mutex<SelectionSnapshot>,
    writes: Mutex<Vec<(Uuid, String)>>,
    gate: (Mutex<GateState>, Condvar),
    point: GatePoint,
}

impl BlockingClaimSelection {
    fn new(current: SelectionSnapshot, point: GatePoint) -> Self {
        Self {
            current: Mutex::new(current),
            writes: Mutex::new(Vec::new()),
            gate: (Mutex::new(GateState::default()), Condvar::new()),
            point,
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

impl SelectionPort for BlockingClaimSelection {
    fn permission_granted(&self, _prompt: bool) -> bool {
        true
    }

    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        Ok(self.current.lock().unwrap().clone())
    }

    fn replace(&self, _expected: &SelectionSnapshot, _text: &str) -> Result<(), VerbalixError> {
        panic!("coordinator must use replace_guarded")
    }

    fn replace_guarded(
        &self,
        expected: &SelectionSnapshot,
        text: &str,
        lease: &TransformLease,
    ) -> Result<(), VerbalixError> {
        if matches!(self.point, GatePoint::BeforeClaim) {
            self.enter_and_wait();
        }
        if !lease.try_claim_write() {
            return Err(VerbalixError::StaleSelection);
        }
        if matches!(self.point, GatePoint::AfterClaim) {
            self.enter_and_wait();
        }
        self.writes
            .lock()
            .unwrap()
            .push((expected.id, text.to_owned()));
        Ok(())
    }

    fn restore(
        &self,
        _expected: &SelectionSnapshot,
        _transformed_text: &str,
    ) -> Result<(), VerbalixError> {
        Ok(())
    }
}

fn ready_blocked(
    point: GatePoint,
) -> (
    Arc<SelectionCoordinator>,
    Arc<BlockingClaimSelection>,
    Arc<EventOverlay>,
    SelectionSnapshot,
) {
    let captured = snapshot(42, "editor-a");
    let selection = Arc::new(BlockingClaimSelection::new(captured.clone(), point));
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
    (coordinator, selection, overlay, captured)
}

fn spawn_transform(
    coordinator: Arc<SelectionCoordinator>,
    captured: SelectionSnapshot,
    input: TransformRequest,
) -> thread::JoinHandle<Result<TransformResult, VerbalixError>> {
    thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(coordinator.transform(captured.id, input, "token", false))
    })
}

fn assert_coordinator_locks_are_available(coordinator: &SelectionCoordinator) {
    assert!(coordinator.state.try_lock().is_ok());
    assert!(coordinator.active_transform.try_lock().is_ok());
}

#[test]
fn candidate_before_write_claim_cancels_without_blocking_or_setter() {
    let (coordinator, selection, _overlay, captured) = ready_blocked(GatePoint::BeforeClaim);
    let input = request();
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();
    let running = spawn_transform(coordinator.clone(), captured, input);
    selection.wait_until_entered();

    assert_coordinator_locks_are_available(&coordinator);
    let next = snapshot(84, "editor-b");
    let next_id = next.id;
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(next)))
        .unwrap();
    selection.release();

    assert!(matches!(
        running.join().unwrap(),
        Err(VerbalixError::StaleSelection)
    ));
    assert!(selection.writes.lock().unwrap().is_empty());
    assert_eq!(coordinator.current_snapshot().unwrap().id, next_id);
}

#[test]
fn invalidation_before_write_claim_cancels_without_blocking_or_setter() {
    let (coordinator, selection, _overlay, captured) = ready_blocked(GatePoint::BeforeClaim);
    let input = request();
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();
    let running = spawn_transform(coordinator.clone(), captured, input);
    selection.wait_until_entered();

    assert_coordinator_locks_are_available(&coordinator);
    coordinator.dispatch(SelectionEvent::Invalidated).unwrap();
    selection.release();

    assert!(matches!(
        running.join().unwrap(),
        Err(VerbalixError::StaleSelection)
    ));
    assert!(selection.writes.lock().unwrap().is_empty());
    assert!(matches!(
        &*coordinator.state.lock().unwrap(),
        SelectionState::Idle
    ));
}

#[test]
fn candidate_before_preview_write_claim_cancels_apply_without_setter() {
    let (coordinator, selection, _overlay, captured) = ready_blocked(GatePoint::BeforeClaim);
    let input = request();
    let request_id = input.request_id;
    coordinator
        .begin_transform(captured.id, request_id)
        .unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(coordinator.transform(captured.id, input, "token", true))
        .unwrap();
    let running = {
        let coordinator = coordinator.clone();
        thread::spawn(move || coordinator.apply_preview(request_id))
    };
    selection.wait_until_entered();

    assert_coordinator_locks_are_available(&coordinator);
    let next = snapshot(84, "editor-b");
    let next_id = next.id;
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(next)))
        .unwrap();
    selection.release();

    assert!(matches!(
        running.join().unwrap(),
        Err(VerbalixError::StaleSelection)
    ));
    assert!(selection.writes.lock().unwrap().is_empty());
    assert_eq!(coordinator.current_snapshot().unwrap().id, next_id);
}

#[test]
fn candidate_after_write_claim_cannot_be_overwritten_by_applied_or_undo() {
    let (coordinator, selection, overlay, captured) = ready_blocked(GatePoint::AfterClaim);
    let input = request();
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();
    let running = spawn_transform(coordinator.clone(), captured, input);
    selection.wait_until_entered();

    assert_coordinator_locks_are_available(&coordinator);
    let next = snapshot(84, "editor-b");
    let next_id = next.id;
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(next)))
        .unwrap();
    selection.release();

    running.join().unwrap().unwrap();
    assert_eq!(selection.writes.lock().unwrap().len(), 1);
    assert_eq!(coordinator.current_snapshot().unwrap().id, next_id);
    assert!(!overlay.events.lock().unwrap().contains(&"undo"));
}
