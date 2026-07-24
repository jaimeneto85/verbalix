use super::execute_if_publishable;
use crate::{
    application::{PublicationPermit, TransformLease},
    platform::{
        note_result::{NoteMode, NoteResultPayload, NoteResultState},
        overlay_readiness::{OverlayReadiness, OverlaySurface},
    },
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    thread,
};
use uuid::Uuid;

#[derive(Default)]
struct PreparationGate {
    state: Mutex<(bool, bool)>,
    changed: Condvar,
}

impl PreparationGate {
    fn enter_and_wait(&self) {
        let mut state = self.state.lock().unwrap();
        state.0 = true;
        self.changed.notify_all();
        while !state.1 {
            state = self.changed.wait(state).unwrap();
        }
    }

    fn wait_until_entered(&self) {
        let mut state = self.state.lock().unwrap();
        while !state.0 {
            state = self.changed.wait(state).unwrap();
        }
    }

    fn release(&self) {
        let mut state = self.state.lock().unwrap();
        state.1 = true;
        self.changed.notify_all();
    }
}

#[derive(Clone, Copy)]
enum Surface {
    Toolbar,
    Note,
}

impl Surface {
    fn overlay(self) -> OverlaySurface {
        match self {
            Self::Toolbar => OverlaySurface::Toolbar,
            Self::Note => OverlaySurface::Note,
        }
    }
}

fn cancellation_during_preparation(surface: Surface) {
    let guard = Arc::new(TransformLease::new(Uuid::new_v4(), Uuid::new_v4()));
    let first_permit = PublicationPermit::new(guard.clone());
    assert!(execute_if_publishable(Some(&first_permit), || Ok(()), || Ok(()), || Ok(()),).unwrap());
    let second_permit = PublicationPermit::new(guard.clone());
    let gate = Arc::new(PreparationGate::default());
    let readiness = Arc::new(OverlayReadiness::default());
    let generation = readiness.begin_document(surface.overlay()).unwrap();
    let emitted = Arc::new(AtomicUsize::new(0));
    let shown = Arc::new(AtomicUsize::new(0));
    let cleaned = Arc::new(AtomicUsize::new(0));
    let note_state = Arc::new(NoteResultState::default());
    if matches!(surface, Surface::Note) {
        note_state
            .publish(
                NoteResultPayload {
                    mode: NoteMode::Result,
                    request_id: None,
                    text: "stale".to_owned(),
                },
                Some(guard.clone()),
            )
            .unwrap();
    }
    let worker = {
        let permit = second_permit.clone();
        let gate = gate.clone();
        let readiness = readiness.clone();
        let emitted = emitted.clone();
        let shown = shown.clone();
        let cleaned = cleaned.clone();
        thread::spawn(move || {
            execute_if_publishable(
                Some(&permit),
                || {
                    readiness.request(surface.overlay())?;
                    gate.enter_and_wait();
                    Ok(())
                },
                || {
                    if matches!(surface, Surface::Note) {
                        emitted.fetch_add(1, Ordering::SeqCst);
                    }
                    shown.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
                || {
                    readiness.cancel(surface.overlay())?;
                    cleaned.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
            )
        })
    };
    gate.wait_until_entered();
    guard.cancel();
    gate.release();

    assert!(!worker.join().unwrap().unwrap());
    assert!(readiness.mark_ready(surface.overlay(), generation).unwrap());
    assert!(!readiness.should_show(surface.overlay()).unwrap());
    assert_eq!(emitted.load(Ordering::SeqCst), 0);
    assert_eq!(shown.load(Ordering::SeqCst), 0);
    assert_eq!(cleaned.load(Ordering::SeqCst), 1);
    assert_eq!(note_state.current().unwrap(), None);
}

#[test]
fn toolbar_cancelled_during_preparation_never_shows() {
    cancellation_during_preparation(Surface::Toolbar);
}

#[test]
fn note_cancelled_during_preparation_never_emits_or_shows() {
    cancellation_during_preparation(Surface::Note);
}

#[test]
fn publication_claim_is_the_boundary_before_final_effects() {
    let guard = Arc::new(TransformLease::new(Uuid::new_v4(), Uuid::new_v4()));
    let permit = PublicationPermit::new(guard.clone());
    let published = AtomicUsize::new(0);

    assert!(execute_if_publishable(
        Some(&permit),
        || Ok(()),
        || {
            published.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
        || Ok(()),
    )
    .unwrap());
    guard.cancel();

    assert_eq!(published.load(Ordering::SeqCst), 1);
    assert!(!guard.may_publish());
}

#[test]
fn cancellation_after_claim_allows_one_publication_before_serial_hide() {
    let guard = Arc::new(TransformLease::new(Uuid::new_v4(), Uuid::new_v4()));
    let first_permit = PublicationPermit::new(guard.clone());
    assert!(execute_if_publishable(Some(&first_permit), || Ok(()), || Ok(()), || Ok(()),).unwrap());
    let second_permit = PublicationPermit::new(guard.clone());
    let gate = Arc::new(PreparationGate::default());
    let effects = Arc::new(Mutex::new(Vec::new()));
    let visible = Arc::new(Mutex::new(false));
    let worker = {
        let permit = second_permit.clone();
        let gate = gate.clone();
        let effects = effects.clone();
        let visible = visible.clone();
        thread::spawn(move || {
            execute_if_publishable(
                Some(&permit),
                || Ok(()),
                || {
                    gate.enter_and_wait();
                    *visible.lock().unwrap() = true;
                    effects.lock().unwrap().push("publish");
                    Ok(())
                },
                || Ok(()),
            )
        })
    };
    gate.wait_until_entered();
    guard.cancel();
    gate.release();

    assert!(worker.join().unwrap().unwrap());
    *visible.lock().unwrap() = false;
    effects.lock().unwrap().push("hide");

    assert_eq!(effects.lock().unwrap().as_slice(), &["publish", "hide"]);
    assert!(!*visible.lock().unwrap());
    assert!(!guard.may_publish());
}
