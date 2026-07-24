use crate::{
    diagnostics,
    domain::VerbalixError,
    platform::{
        overlay_dispatcher::{place, OverlayCommand},
        overlay_publication::execute_if_publishable,
        overlay_readiness::{OverlayReadiness, OverlaySurface},
        overlay_window::{get_or_create, show_if_ready},
    },
};
use std::{cell::RefCell, sync::Arc};
use tauri::{AppHandle, Emitter, Manager};

pub(super) fn execute_command(
    app: &AppHandle,
    command: OverlayCommand,
    sequence: u64,
    readiness: &Arc<OverlayReadiness>,
) -> Result<(), VerbalixError> {
    match command {
        OverlayCommand::ShowToolbar(bounds, guard) => {
            let surface = OverlaySurface::Toolbar;
            let prepared_window = RefCell::new(None);
            let executed = execute_if_publishable(
                guard.as_ref(),
                || {
                    readiness.request(surface)?;
                    let window = get_or_create(app, surface, 236.0, 52.0, sequence, readiness)?;
                    place(app, &window, bounds, 236.0, 52.0, "toolbar", sequence)?;
                    prepared_window.replace(Some(window));
                    Ok(())
                },
                || {
                    let window = prepared_window
                        .borrow()
                        .as_ref()
                        .cloned()
                        .ok_or(VerbalixError::LocalFailure)?;
                    show_if_ready(&window, surface, sequence, readiness)
                },
                || readiness.cancel(surface),
            )?;
            record_cancellation(executed, "toolbar", sequence);
            Ok(())
        }
        OverlayCommand::ShowResult(bounds, payload, guard) => {
            let surface = OverlaySurface::Note;
            let prepared_window = RefCell::new(None);
            let executed = execute_if_publishable(
                guard.as_ref(),
                || {
                    readiness.request(surface)?;
                    let window = get_or_create(app, surface, 420.0, 220.0, sequence, readiness)?;
                    place(app, &window, bounds, 420.0, 220.0, "note", sequence)?;
                    prepared_window.replace(Some(window));
                    Ok(())
                },
                || {
                    let window = prepared_window
                        .borrow()
                        .as_ref()
                        .cloned()
                        .ok_or(VerbalixError::LocalFailure)?;
                    window
                        .emit("note-result", payload)
                        .map_err(|_| VerbalixError::LocalFailure)?;
                    diagnostics::overlay("emitted", "note", sequence);
                    show_if_ready(&window, surface, sequence, readiness)
                },
                || readiness.cancel(surface),
            )?;
            record_cancellation(executed, "note", sequence);
            Ok(())
        }
        OverlayCommand::HideAll => hide_all(app, sequence, readiness),
    }
}

fn record_cancellation(executed: bool, label: &'static str, sequence: u64) {
    if !executed {
        diagnostics::overlay("cancelled", label, sequence);
    }
}

fn hide_all(
    app: &AppHandle,
    sequence: u64,
    readiness: &OverlayReadiness,
) -> Result<(), VerbalixError> {
    readiness.cancel_all()?;
    for label in ["toolbar", "note"] {
        if let Some(window) = app.get_webview_window(label) {
            window.hide().map_err(|_| VerbalixError::LocalFailure)?;
            let visible = window
                .is_visible()
                .map_err(|_| VerbalixError::LocalFailure)?;
            diagnostics::overlay_visibility(label, sequence, visible);
            if visible {
                return Err(VerbalixError::LocalFailure);
            }
        }
    }
    Ok(())
}
