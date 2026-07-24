use crate::{commands::route_refresh_failure, domain::VerbalixError};
use std::cell::Cell;

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
