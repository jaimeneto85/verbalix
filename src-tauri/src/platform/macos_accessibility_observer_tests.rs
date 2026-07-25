use super::*;
use crate::platform::macos_ax::AxElementToken;
use std::cell::Cell;

fn token(hash: usize) -> AxElementToken {
    AxElementToken { pid: 42, hash }
}

fn event(kind: AccessibilityEventKind, target: Option<AxElementToken>) -> AccessibilityEvent {
    AccessibilityEvent { kind, target }
}

#[test]
fn exact_self_change_is_suppressed_once_and_next_external_change_bumps_epoch() {
    let epoch = CausalEpoch::default();
    let generation = epoch.current();
    let classifications = Cell::new(0);

    let own = route_observer_event(
        event(AccessibilityEventKind::SelectedTextChanged, Some(token(7))),
        &epoch,
        |target, observed_generation| {
            classifications.set(classifications.get() + 1);
            assert_eq!(target, token(7));
            assert_eq!(observed_generation, generation);
            Ok(ObservedSelectionChange::SelfGenerated)
        },
    );
    assert!(!own);
    assert!(epoch.is_current(generation));

    let external = route_observer_event(
        event(AccessibilityEventKind::SelectedTextChanged, Some(token(7))),
        &epoch,
        |_, _| Ok(ObservedSelectionChange::External),
    );
    assert!(external);
    assert!(!epoch.is_current(generation));
    assert_eq!(classifications.get(), 1);
}

#[test]
fn target_mismatch_or_missing_target_is_never_suppressed() {
    for target in [Some(token(8)), None] {
        let epoch = CausalEpoch::default();
        let generation = epoch.current();
        let routed = route_observer_event(
            event(AccessibilityEventKind::SelectedTextChanged, target),
            &epoch,
            |_, _| Ok(ObservedSelectionChange::External),
        );
        assert!(routed);
        assert!(!epoch.is_current(generation));
    }
}

#[test]
fn focus_and_destroy_always_bump_before_callback_without_classification() {
    for kind in [
        AccessibilityEventKind::FocusChanged,
        AccessibilityEventKind::ElementDestroyed,
    ] {
        let epoch = CausalEpoch::default();
        let generation = epoch.current();
        let classified = Cell::new(false);
        let routed = route_observer_event(event(kind, Some(token(7))), &epoch, |_, _| {
            classified.set(true);
            Ok(ObservedSelectionChange::SelfGenerated)
        });
        assert!(routed);
        assert!(!epoch.is_current(generation));
        assert!(!classified.get());
    }
}
