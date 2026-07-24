use super::*;

#[tokio::test]
async fn provider_completion_after_supersede_is_inert() {
    let provider = Arc::new(BlockingProvider {
        calls: Mutex::new(0),
        started: Notify::new(),
        release: Notify::new(),
    });
    let (coordinator, selection, overlay, captured) = ready(provider.clone(), false);
    let input = request();
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();
    let running = {
        let coordinator = coordinator.clone();
        tokio::spawn(async move {
            coordinator
                .transform(captured.id, input, "token", false)
                .await
        })
    };
    provider.started.notified().await;
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(snapshot(
            84, "editor-b",
        ))))
        .unwrap();
    provider.release.notify_one();

    assert!(matches!(
        running.await.unwrap(),
        Err(VerbalixError::StaleSelection)
    ));
    assert_eq!(*provider.calls.lock().unwrap(), 1);
    assert!(selection.writes.lock().unwrap().is_empty());
    assert_eq!(
        overlay.events.lock().unwrap().as_slice(),
        ["toolbar", "hide"]
    );
}

#[test]
fn transient_invalidation_preserves_but_real_invalidation_cancels_the_lease() {
    let provider = Arc::new(ImmediateProvider::default());
    let (coordinator, _selection, overlay, captured) = ready(provider, false);
    let input = request();
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();
    coordinator
        .dispatch(SelectionEvent::TransientInvalidated)
        .unwrap();
    assert!(matches!(
        &*coordinator.state.lock().unwrap(),
        SelectionState::Processing { request_id, .. } if *request_id == input.request_id
    ));

    coordinator.dispatch(SelectionEvent::Invalidated).unwrap();
    assert!(matches!(
        &*coordinator.state.lock().unwrap(),
        SelectionState::Idle
    ));
    assert_eq!(
        overlay.events.lock().unwrap().as_slice(),
        ["toolbar", "hide"]
    );
}

#[test]
fn failed_hide_does_not_restore_a_superseded_processing_lease() {
    let provider = Arc::new(ImmediateProvider::default());
    let (coordinator, _selection, overlay, captured) = ready(provider, true);
    let input = request();
    coordinator
        .begin_transform(captured.id, input.request_id)
        .unwrap();
    let next = snapshot(84, "editor-b");
    let next_id = next.id;

    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(next)))
        .unwrap();

    assert_eq!(coordinator.current_snapshot().unwrap().id, next_id);
    assert!(matches!(
        &*coordinator.state.lock().unwrap(),
        SelectionState::Candidate(snapshot) if snapshot.id == next_id
    ));
    assert!(overlay.events.lock().unwrap().contains(&"hide"));
}

#[tokio::test]
async fn supersede_before_preview_apply_fails_closed_without_a_write() {
    let provider = Arc::new(ImmediateProvider::default());
    let (coordinator, selection, _overlay, captured) = ready(provider, false);
    let input = request();
    let request_id = input.request_id;
    coordinator
        .begin_transform(captured.id, request_id)
        .unwrap();
    coordinator
        .transform(captured.id, input, "token", true)
        .await
        .unwrap();
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(snapshot(
            84, "editor-b",
        ))))
        .unwrap();

    assert!(matches!(
        coordinator.apply_preview(request_id),
        Err(VerbalixError::StaleSelection)
    ));
    assert!(selection.writes.lock().unwrap().is_empty());
}
