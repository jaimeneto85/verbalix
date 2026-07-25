use super::{error_bounds_with, route_refresh_failure};
use crate::{
    application::{
        set_test_clock_ms, test_clock_ms, OverlayPort, RuntimePause, SelectionCoordinator,
        SelectionPort,
    },
    domain::{
        AiProvider, Rect, SelectionEvent, SelectionSnapshot, TextRange, TransformRequest,
        TransformResult, VerbalixError,
    },
};
use async_trait::async_trait;
use std::{cell::Cell, sync::Mutex};
use uuid::Uuid;

fn effect_counts(error: VerbalixError) -> (u32, u32) {
    let login_window_calls = Cell::new(0);
    let provider_note_calls = Cell::new(0);
    route_refresh_failure(
        &error,
        || login_window_calls.set(login_window_calls.get() + 1),
        || provider_note_calls.set(provider_note_calls.get() + 1),
    );
    (login_window_calls.get(), provider_note_calls.get())
}

#[test]
fn existing_session_network_refresh_failure_never_opens_login() {
    assert_eq!(
        effect_counts(VerbalixError::ProviderRejected),
        (0, 1),
        "provider_unavailable must be visible without opening the main window"
    );
}

#[test]
fn existing_session_invalid_refresh_response_never_opens_login() {
    assert_eq!(
        effect_counts(VerbalixError::InvalidResponse),
        (0, 1),
        "provider_unavailable must be visible without opening the main window"
    );
}

#[test]
fn expired_session_refresh_opens_login_without_provider_error() {
    assert_eq!(effect_counts(VerbalixError::Unauthenticated), (1, 0));
}

struct StubSelection {
    snapshot: Mutex<SelectionSnapshot>,
}

impl SelectionPort for StubSelection {
    fn permission_granted(&self, _prompt: bool) -> bool {
        true
    }

    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        Ok(self.snapshot.lock().unwrap().clone())
    }

    fn replace(&self, _expected: &SelectionSnapshot, text: &str) -> Result<(), VerbalixError> {
        self.snapshot.lock().unwrap().text = text.to_owned();
        Ok(())
    }

    fn restore(&self, _expected: &SelectionSnapshot, _original: &str) -> Result<(), VerbalixError> {
        Ok(())
    }
}

#[derive(Default)]
struct StubOverlay;

impl OverlayPort for StubOverlay {
    fn show_toolbar(&self, _bounds: Rect) -> Result<(), VerbalixError> {
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
        Ok(())
    }
}

struct StubProvider;

#[async_trait]
impl AiProvider for StubProvider {
    async fn transform(
        &self,
        request: &TransformRequest,
        _token: &str,
    ) -> Result<TransformResult, VerbalixError> {
        Ok(TransformResult {
            request_id: request.request_id,
            source_language: "pt".to_owned(),
            target_language: Some("en".to_owned()),
            result: "stub".to_owned(),
        })
    }
}

fn cmd_snapshot() -> SelectionSnapshot {
    SelectionSnapshot::new(
        1,
        "com.test".to_owned(),
        "text".to_owned(),
        TextRange {
            location: 0,
            length: 4,
        },
        Rect {
            x: 5.0,
            y: 10.0,
            width: 30.0,
            height: 10.0,
        },
        true,
    )
}

fn make_coordinator_with_snapshot(snap: SelectionSnapshot) -> SelectionCoordinator {
    use std::sync::Arc;
    SelectionCoordinator::new(
        Arc::new(StubSelection {
            snapshot: Mutex::new(snap),
        }),
        Arc::new(StubOverlay),
        Arc::new(StubProvider),
    )
}

#[test]
fn error_bounds_with_returns_none_when_no_snapshot_and_no_in_flight() {
    set_test_clock_ms(0);
    let snap = cmd_snapshot();
    let coordinator = make_coordinator_with_snapshot(snap);
    let pause = RuntimePause::with_clock(test_clock_ms);

    assert!(error_bounds_with(&coordinator, &pause).is_none());
}

#[test]
fn error_bounds_with_returns_current_snapshot_bounds_when_in_flight_and_candidate_state() {
    set_test_clock_ms(0);
    let snap = cmd_snapshot();
    let expected_bounds = snap.bounds;
    let coordinator = make_coordinator_with_snapshot(snap.clone());
    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(snap)))
        .unwrap();

    assert!(coordinator.current_snapshot().is_some());

    let pause = RuntimePause::with_clock(test_clock_ms);
    let _guard = pause.begin_action();

    assert_eq!(
        error_bounds_with(&coordinator, &pause),
        Some(expected_bounds)
    );
}

#[test]
fn error_bounds_with_returns_last_known_bounds_fallback_when_idle_in_flight_and_bounds_present() {
    set_test_clock_ms(0);
    let snap = cmd_snapshot();
    let expected_bounds = snap.bounds;
    let coordinator = make_coordinator_with_snapshot(snap.clone());

    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(snap)))
        .unwrap();
    coordinator.dispatch(SelectionEvent::Invalidated).unwrap();

    assert!(coordinator.current_snapshot().is_none());
    assert!(coordinator.last_known_bounds().is_none());

    coordinator.force_last_bounds_for_test(Some(expected_bounds));
    assert_eq!(coordinator.last_known_bounds(), Some(expected_bounds));

    let pause = RuntimePause::with_clock(test_clock_ms);
    let _guard = pause.begin_action();

    assert_eq!(
        error_bounds_with(&coordinator, &pause),
        Some(expected_bounds)
    );
}

#[test]
fn error_bounds_with_returns_none_when_idle_not_in_flight_even_with_last_bounds() {
    set_test_clock_ms(0);
    let snap = cmd_snapshot();
    let coordinator = make_coordinator_with_snapshot(snap.clone());

    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(snap.clone())))
        .unwrap();
    coordinator.dispatch(SelectionEvent::Invalidated).unwrap();

    coordinator.force_last_bounds_for_test(Some(snap.bounds));

    let pause = RuntimePause::with_clock(test_clock_ms);

    assert!(error_bounds_with(&coordinator, &pause).is_none());
}

#[test]
fn coordinator_invalidated_dispatches_regardless_of_in_flight_state() {
    set_test_clock_ms(0);
    let snap = cmd_snapshot();
    let coordinator = make_coordinator_with_snapshot(snap.clone());
    let pause = RuntimePause::with_clock(test_clock_ms);

    coordinator
        .dispatch(SelectionEvent::Candidate(Box::new(snap.clone())))
        .unwrap();
    coordinator
        .dispatch(SelectionEvent::DebounceElapsed(snap.id))
        .unwrap();

    let _guard = pause.begin_action();
    assert!(pause.is_action_in_flight());

    coordinator.dispatch(SelectionEvent::Invalidated).unwrap();

    assert!(coordinator.current_snapshot().is_none());
    assert!(coordinator.last_known_bounds().is_none());
}

#[test]
fn current_selection_command_dto_never_serializes_native_ax_identity() {
    let sentinel = "private-command-ax-identifier";
    let command_result = Some(
        SelectionSnapshot::new(
            42,
            "pid:42".to_owned(),
            "selected".to_owned(),
            TextRange {
                location: 0,
                length: 8,
            },
            Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            },
            true,
        )
        .with_native_element_identifier(Some(sentinel.to_owned())),
    );
    let dto = serde_json::to_string(&command_result).unwrap();
    assert!(!dto.contains(sentinel));
    assert!(!dto.contains("identifier"));
}
