use super::*;

#[tokio::test]
async fn concurrent_request_is_rejected_while_first_remains_active() {
    let captured = snapshot(true);
    let selection = Arc::new(FakeSelection {
        snapshot: Mutex::new(captured.clone()),
        replacements: Mutex::new(Vec::new()),
        fail_write: false,
    });
    let provider = Arc::new(OutOfOrderProvider {
        calls: AtomicUsize::new(0),
        started: Notify::new(),
        release: Notify::new(),
    });
    let coordinator = Arc::new(SelectionCoordinator::new(
        selection.clone(),
        Arc::new(FakeOverlay::default()),
        provider.clone(),
    ));
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(captured.clone())))
        .unwrap();
    coordinator
        .dispatch(SelectionEvent::DebounceElapsed(captured.id))
        .unwrap();
    let first_coordinator = coordinator.clone();
    let first = tokio::spawn(async move {
        execute(&first_coordinator, request("Olá 👩🏽‍💻"), false).await
    });
    provider.started.notified().await;
    assert!(matches!(
        execute(&coordinator, request("Olá 👩🏽‍💻"), false).await,
        Err(VerbalixError::OperationInProgress)
    ));
    provider.release.notify_one();
    first.await.unwrap().unwrap();
    assert_eq!(
        selection.replacements.lock().unwrap().as_slice(),
        ["result-0"]
    );
}

#[test]
fn oversized_selection_is_invalidated() {
    let (coordinator, selection, overlay) = ready(true, false, false);
    selection.snapshot.lock().unwrap().text = "a".repeat(12_001);
    assert!(matches!(
        coordinator.refresh_selection(),
        Err(VerbalixError::TextTooLong)
    ));
    assert!(overlay.events.lock().unwrap().contains(&"hide".to_owned()));
}
