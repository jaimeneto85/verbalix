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
    let generation = readiness.begin_document(surface)?;
    let document_started = Arc::new(AtomicBool::new(false));
    let reload_started = document_started.clone();
    let reload_readiness = readiness.clone();
    let document_url = overlay_document_url(surface, generation);
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(document_url.into()))
        .on_page_load(move |window, payload| {
            if payload.event() == PageLoadEvent::Started
                && reload_started.swap(true, Ordering::AcqRel)
            {
                match reload_readiness.invalidate_document(surface) {
                    Ok(()) => diagnostics::overlay("reload_invalidated", label, sequence),
                    Err(_) => diagnostics::overlay("reload_invalidation_failed", label, sequence),
                }
                match window.destroy() {
                    Ok(()) => diagnostics::overlay("reload_window_destroyed", label, sequence),
                    Err(_) => {
                        diagnostics::overlay("reload_destroy_failed", label, sequence);
                        match window.hide() {
                            Ok(()) => diagnostics::overlay("reload_window_hidden", label, sequence),
                            Err(_) => diagnostics::overlay("reload_hide_failed", label, sequence),
                        }
                    }
                }
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
        .map_err(|_| VerbalixError::LocalFailure)?;
    #[cfg(target_os = "macos")]
    macos_overlay_panel::configure(&window)?;
    diagnostics::overlay("window_created", label, sequence);
    Ok(window)
}

fn overlay_document_url(surface: OverlaySurface, generation: uuid::Uuid) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reload_destroy_then_recreate_uses_a_fresh_generation_and_ack() {
        let readiness = OverlayReadiness::default();
        let surface = OverlaySurface::Toolbar;
        let old_generation = readiness.begin_document(surface).unwrap();
        let old_url = overlay_document_url(surface, old_generation);
        readiness.request(surface).unwrap();
        assert!(readiness.mark_ready(surface, old_generation).unwrap());
        assert!(readiness.should_show(surface).unwrap());

        readiness.invalidate_document(surface).unwrap();
        assert!(!readiness.mark_ready(surface, old_generation).unwrap());
        assert!(!readiness.should_show(surface).unwrap());

        let new_generation = readiness.begin_document(surface).unwrap();
        let new_url = overlay_document_url(surface, new_generation);
        assert_ne!(new_generation, old_generation);
        assert_ne!(new_url, old_url);
        assert_eq!(new_generation.get_version_num(), 4);
        assert!(!readiness.mark_ready(surface, old_generation).unwrap());
        assert!(readiness.mark_ready(surface, new_generation).unwrap());
        assert!(readiness.should_show(surface).unwrap());
    }
}
