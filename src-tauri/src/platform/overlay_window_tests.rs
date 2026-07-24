use super::{
    overlay_readiness::{OverlayReadiness, OverlaySurface},
    overlay_window::{create_configured_document, overlay_document_url},
};
use crate::domain::VerbalixError;
use std::cell::{Cell, RefCell};

#[test]
fn reload_destroy_then_recreate_uses_a_fresh_generation_and_ack() {
    let readiness = OverlayReadiness::default();
    let surface = OverlaySurface::Toolbar;
    let old_generation = readiness.begin_document(surface).unwrap();
    let old_url = overlay_document_url(surface, old_generation);
    readiness.request(surface).unwrap();
    assert!(readiness.mark_ready(surface, old_generation).unwrap());

    readiness.invalidate_document(surface).unwrap();
    assert!(!readiness.mark_ready(surface, old_generation).unwrap());
    assert!(!readiness.should_show(surface).unwrap());

    let new_generation = readiness.begin_document(surface).unwrap();
    let new_url = overlay_document_url(surface, new_generation);
    assert_ne!(new_generation, old_generation);
    assert_ne!(new_url, old_url);
    assert_eq!(new_generation.get_version_num(), 4);
    assert!(readiness.mark_ready(surface, new_generation).unwrap());
    assert!(readiness.should_show(surface).unwrap());
}

#[test]
fn configure_failure_rolls_back_before_a_fresh_creation() {
    let readiness = OverlayReadiness::default();
    let generations = RefCell::new(Vec::new());
    let destroyed = Cell::new(false);

    let failed = create_configured_document(
        &readiness,
        OverlaySurface::Toolbar,
        1,
        |generation| {
            generations.borrow_mut().push(generation);
            Ok("unconfigured")
        },
        |_| Err(VerbalixError::LocalFailure),
        |_| {
            destroyed.set(true);
            Ok(())
        },
        |_| Ok(()),
    );

    assert!(failed.is_err());
    assert!(destroyed.get());
    assert!(!readiness.has_document(OverlaySurface::Toolbar).unwrap());

    let created = create_configured_document(
        &readiness,
        OverlaySurface::Toolbar,
        2,
        |generation| {
            generations.borrow_mut().push(generation);
            Ok("configured")
        },
        |_| Ok(()),
        |_| Ok(()),
        |_| Ok(()),
    )
    .unwrap();

    assert_eq!(created.0, "configured");
    assert_ne!(generations.borrow()[0], generations.borrow()[1]);
    assert!(readiness.has_document(OverlaySurface::Toolbar).unwrap());
}

#[test]
fn failed_destroy_and_hide_still_leave_creation_invalidated() {
    let readiness = OverlayReadiness::default();
    let destroy_attempted = Cell::new(false);
    let hide_attempted = Cell::new(false);

    let result = create_configured_document(
        &readiness,
        OverlaySurface::Note,
        3,
        |_| Ok("unconfigured"),
        |_| Err(VerbalixError::LocalFailure),
        |_| {
            destroy_attempted.set(true);
            Err(VerbalixError::LocalFailure)
        },
        |_| {
            hide_attempted.set(true);
            Err(VerbalixError::LocalFailure)
        },
    );

    assert!(result.is_err());
    assert!(destroy_attempted.get());
    assert!(hide_attempted.get());
    assert!(!readiness.has_document(OverlaySurface::Note).unwrap());
}
