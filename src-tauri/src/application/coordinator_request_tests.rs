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

fn different_candidate() -> SelectionSnapshot {
    let mut candidate = snapshot(true);
    candidate.pid = 84;
    candidate
}

#[tokio::test]
async fn candidate_during_readiness_revokes_pin_before_provider_or_feedback() {
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
    let coordinator = SelectionCoordinator::new(
        selection.clone(),
        Arc::new(FakeOverlay::default()),
        provider.clone(),
    );
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(captured.clone())))
        .unwrap();
    coordinator
        .dispatch(SelectionEvent::DebounceElapsed(captured.id))
        .unwrap();
    let input = request(&captured.text);
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();

    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(different_candidate())))
        .unwrap();
    assert!(matches!(
        coordinator
            .transform(captured.id, input.clone(), "token", false)
            .await,
        Err(VerbalixError::StaleSelection)
    ));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(selection.replacements.lock().unwrap().is_empty());
    assert!(coordinator
        .publication_guard(captured.id, input.request_id)
        .unwrap()
        .is_none());
}

#[test]
fn begin_errors_keep_typed_feedback_only_until_candidate_supersedes() {
    let (coordinator, _selection, _overlay) = ready(true, false, false);
    let snapshot_id = coordinator.current_snapshot().unwrap().id;
    let stale_guard = coordinator.feedback_guard(snapshot_id).unwrap().unwrap();
    assert!(matches!(
        coordinator.begin_transform(Uuid::new_v4(), Uuid::new_v4()),
        Err(VerbalixError::StaleSelection)
    ));
    coordinator
        .begin_transform(snapshot_id, Uuid::new_v4())
        .unwrap();
    let active_guard = coordinator.feedback_guard(snapshot_id).unwrap().unwrap();
    assert!(matches!(
        coordinator.begin_transform(snapshot_id, Uuid::new_v4()),
        Err(VerbalixError::OperationInProgress)
    ));

    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(different_candidate())))
        .unwrap();
    assert!(!stale_guard.may_publish());
    assert!(!active_guard.may_publish());
}

#[tokio::test]
async fn apply_and_undo_failures_expose_guards_that_candidate_can_cancel() {
    let (preview, _selection, _overlay) = ready(true, true, false);
    let input = request("Olá 👩🏽‍💻");
    let request_id = input.request_id;
    execute(&preview, input, true).await.unwrap();
    let (_, apply_guard) = preview.preview_feedback(request_id).unwrap().unwrap();
    assert!(matches!(
        preview.apply_preview(request_id),
        Err(VerbalixError::StaleSelection)
    ));
    preview
        .dispatch(SelectionEvent::Candidate(Box::new(different_candidate())))
        .unwrap();
    assert!(!apply_guard.may_publish());

    let (undo, selection, _overlay) = ready(true, false, false);
    execute(&undo, request("Olá 👩🏽‍💻"), false).await.unwrap();
    let (_, undo_guard) = undo.undo_feedback("result").unwrap().unwrap();
    selection.snapshot.lock().unwrap().text = "changed".to_owned();
    assert!(matches!(
        undo.undo("result"),
        Err(VerbalixError::StaleSelection)
    ));
    undo.dispatch(SelectionEvent::Candidate(Box::new(different_candidate())))
        .unwrap();
    assert!(!undo_guard.may_publish());
}
