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
        diagnostics::overlay("window_reused", label, sequence);
        return Ok(window);
    }
    let generation = readiness.begin_document(surface)?;
    let document_started = Arc::new(AtomicBool::new(false));
    let reload_started = document_started.clone();
    let reload_readiness = readiness.clone();
    let window = WebviewWindowBuilder::new(
        app,
        label,
        WebviewUrl::App(format!("index.html?overlay={label}&generation={generation}").into()),
    )
    .on_page_load(move |window, payload| {
        if payload.event() == PageLoadEvent::Started && reload_started.swap(true, Ordering::AcqRel)
        {
            let _ = reload_readiness.begin_document(surface);
            let _ = window.hide();
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
