use super::*;
use crate::application::TransformLease;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn actor_is_send_sync_without_sending_ax_handles() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<AxActor>();
}

#[test]
fn external_epoch_revokes_blocked_write_before_pending_capture_runs() {
    let actor = AxActor::new();
    let observed_epoch = actor.epoch.current();
    let lease = std::sync::Arc::new(TransformLease::new(Uuid::new_v4(), Uuid::new_v4()));
    let lease_probe = lease.clone();
    let setters = std::sync::Arc::new(AtomicUsize::new(0));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    actor
        .sender
        .send(Command::TestBoundary {
            observed_epoch,
            lease,
            entered: entered_tx,
            release: release_rx,
            setters: setters.clone(),
            response: result_tx,
        })
        .unwrap();
    entered_rx.recv().unwrap();

    actor.signal_causal_change();
    let (capture_tx, capture_rx) = mpsc::channel();
    actor
        .sender
        .send(Command::TestPendingCapture(capture_tx))
        .unwrap();
    assert!(matches!(
        capture_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert_eq!(setters.load(Ordering::SeqCst), 0);
    release_tx.send(()).unwrap();

    assert!(matches!(
        result_rx.recv().unwrap(),
        Err(VerbalixError::StaleSelection)
    ));
    capture_rx.recv().unwrap();
    assert_eq!(setters.load(Ordering::SeqCst), 0);
    assert!(
        lease_probe.try_claim_write(),
        "epoch must reject A before consuming its write claim"
    );
}

#[test]
fn stable_epoch_writes_once_before_pending_capture_runs() {
    let actor = AxActor::new();
    let observed_epoch = actor.epoch.current();
    let lease = std::sync::Arc::new(TransformLease::new(Uuid::new_v4(), Uuid::new_v4()));
    let setters = std::sync::Arc::new(AtomicUsize::new(0));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    actor
        .sender
        .send(Command::TestBoundary {
            observed_epoch,
            lease,
            entered: entered_tx,
            release: release_rx,
            setters: setters.clone(),
            response: result_tx,
        })
        .unwrap();
    entered_rx.recv().unwrap();
    let (capture_tx, capture_rx) = mpsc::channel();
    actor
        .sender
        .send(Command::TestPendingCapture(capture_tx))
        .unwrap();
    assert!(matches!(
        capture_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));

    release_tx.send(()).unwrap();

    result_rx.recv().unwrap().unwrap();
    assert_eq!(setters.load(Ordering::SeqCst), 1);
    capture_rx.recv().unwrap();
}
