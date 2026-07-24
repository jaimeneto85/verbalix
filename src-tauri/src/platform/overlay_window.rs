#[cfg(target_os = "macos")]
use crate::platform::macos_overlay_panel;
use crate::{
    diagnostics,
    domain::VerbalixError,
    platform::overlay_readiness::{OverlayReadiness, OverlaySurface},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{
    webview::PageLoadEvent, AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

pub fn get_or_create(
    app: &AppHandle,
    surface: OverlaySurface,
    width: f64,
    height: f64,
    sequence: u64,
    readiness: &Arc<OverlayReadiness>,
) -> Result<WebviewWindow, VerbalixError> {
    let label = surface.label();
    if let Some(window) = app.get_webview_window(label) {
        if readiness.has_document(surface)? {
            diagnostics::overlay("window_reused", label, sequence);
            return Ok(window);
        }
        diagnostics::overlay("invalid_window_found", label, sequence);
        window.destroy().map_err(|_| {
            diagnostics::overlay("invalid_window_destroy_failed", label, sequence);
            VerbalixError::LocalFailure
        })?;
        diagnostics::overlay("invalid_window_destroyed", label, sequence);
    }
    let (window, _) = create_configured_document(
        readiness,
        surface,
        sequence,
        |generation| {
            let document_started = Arc::new(AtomicBool::new(false));
            let reload_started = document_started.clone();
            let reload_readiness = readiness.clone();
            let document_url = overlay_document_url(surface, generation);
            WebviewWindowBuilder::new(app, label, WebviewUrl::App(document_url.into()))
                .on_page_load(move |window, payload| {
                    if payload.event() == PageLoadEvent::Started
                        && reload_started.swap(true, Ordering::AcqRel)
                    {
                        invalidate_reloaded_window(
                            &window,
                            &reload_readiness,
                            surface,
                            generation,
                            sequence,
                        );
                    }
                })
                .title("Verbalix")
                .inner_size(width, height)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .focused(false)
                .visible(false)
                .build()
                .map_err(|_| VerbalixError::LocalFailure)
        },
        configure_window,
        |window| window.destroy().map_err(|_| VerbalixError::LocalFailure),
        |window| window.hide().map_err(|_| VerbalixError::LocalFailure),
    )?;
    diagnostics::overlay("window_created", label, sequence);
    Ok(window)
}

pub(super) fn create_configured_document<T>(
    readiness: &OverlayReadiness,
    surface: OverlaySurface,
    sequence: u64,
    build: impl FnOnce(uuid::Uuid) -> Result<T, VerbalixError>,
    configure: impl FnOnce(&T) -> Result<(), VerbalixError>,
    destroy: impl FnOnce(&T) -> Result<(), VerbalixError>,
    hide: impl FnOnce(&T) -> Result<(), VerbalixError>,
) -> Result<(T, uuid::Uuid), VerbalixError> {
    let label = surface.label();
    let generation = readiness.begin_document(surface)?;
    let window = match build(generation) {
        Ok(window) => window,
        Err(error) => {
            diagnostics::overlay("window_build_failed", label, sequence);
            invalidate_creation(readiness, surface, generation, sequence);
            return Err(error);
        }
    };
    if let Err(error) = configure(&window) {
        diagnostics::overlay("window_configure_failed", label, sequence);
        invalidate_creation(readiness, surface, generation, sequence);
        rollback_window(&window, label, sequence, destroy, hide);
        return Err(error);
    }
    Ok((window, generation))
}

fn invalidate_creation(
    readiness: &OverlayReadiness,
    surface: OverlaySurface,
    generation: uuid::Uuid,
    sequence: u64,
) {
    match readiness.invalidate_if_current(surface, generation) {
        Ok(true) => diagnostics::overlay("creation_invalidated", surface.label(), sequence),
        Ok(false) => diagnostics::overlay("creation_invalidation_stale", surface.label(), sequence),
        Err(_) => diagnostics::overlay("creation_invalidation_failed", surface.label(), sequence),
    }
}

fn rollback_window<T>(
    window: &T,
    label: &str,
    sequence: u64,
    destroy: impl FnOnce(&T) -> Result<(), VerbalixError>,
    hide: impl FnOnce(&T) -> Result<(), VerbalixError>,
) {
    match destroy(window) {
        Ok(()) => diagnostics::overlay("rollback_window_destroyed", label, sequence),
        Err(_) => {
            diagnostics::overlay("rollback_destroy_failed", label, sequence);
            match hide(window) {
                Ok(()) => diagnostics::overlay("rollback_window_hidden", label, sequence),
                Err(_) => diagnostics::overlay("rollback_hide_failed", label, sequence),
            }
        }
    }
}

fn invalidate_reloaded_window(
    window: &WebviewWindow,
    readiness: &OverlayReadiness,
    surface: OverlaySurface,
    generation: uuid::Uuid,
    sequence: u64,
) {
    let label = surface.label();
    match readiness.invalidate_if_current(surface, generation) {
        Ok(true) => diagnostics::overlay("reload_invalidated", label, sequence),
        Ok(false) => diagnostics::overlay("reload_invalidation_stale", label, sequence),
        Err(_) => diagnostics::overlay("reload_invalidation_failed", label, sequence),
    }
    rollback_window(
        window,
        label,
        sequence,
        |window| window.destroy().map_err(|_| VerbalixError::LocalFailure),
        |window| window.hide().map_err(|_| VerbalixError::LocalFailure),
    );
}

#[cfg(target_os = "macos")]
fn configure_window(window: &WebviewWindow) -> Result<(), VerbalixError> {
    macos_overlay_panel::configure(window)
}

#[cfg(not(target_os = "macos"))]
fn configure_window(_window: &WebviewWindow) -> Result<(), VerbalixError> {
    Ok(())
}

pub(super) fn overlay_document_url(surface: OverlaySurface, generation: uuid::Uuid) -> String {
    format!(
        "index.html?overlay={}&generation={generation}",
        surface.label()
    )
}

pub fn is_current_caller(
    app: &AppHandle,
    caller: &WebviewWindow,
    surface: OverlaySurface,
) -> Result<bool, VerbalixError> {
    if caller.label() != surface.label() {
        return Ok(false);
    }
    let current = app
        .get_webview_window(surface.label())
        .ok_or(VerbalixError::LocalFailure)?;
    #[cfg(target_os = "macos")]
    {
        let caller_view = caller.ns_view().map_err(|_| VerbalixError::LocalFailure)?;
        let current_view = current.ns_view().map_err(|_| VerbalixError::LocalFailure)?;
        Ok(caller_view == current_view)
    }
    #[cfg(not(target_os = "macos"))]
    Ok(caller == &current)
}

pub fn show_if_ready(
    window: &WebviewWindow,
    surface: OverlaySurface,
    sequence: u64,
    readiness: &OverlayReadiness,
) -> Result<(), VerbalixError> {
    if readiness.should_show(surface)? {
        return show_and_confirm(window, surface.label(), sequence);
    }
    if window
        .is_visible()
        .map_err(|_| VerbalixError::LocalFailure)?
    {
        window.hide().map_err(|_| VerbalixError::LocalFailure)?;
    }
    diagnostics::overlay("awaiting_surface", surface.label(), sequence);
    Ok(())
}

fn show_and_confirm(
    window: &WebviewWindow,
    label: &str,
    sequence: u64,
) -> Result<(), VerbalixError> {
    window.show().map_err(|_| VerbalixError::LocalFailure)?;
    let visible = window
        .is_visible()
        .map_err(|_| VerbalixError::LocalFailure)?;
    diagnostics::overlay_visibility(label, sequence, visible);
    if visible {
        Ok(())
    } else {
        Err(VerbalixError::LocalFailure)
    }
}
