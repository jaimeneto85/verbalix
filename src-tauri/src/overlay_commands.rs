use crate::{
    domain::{SelectionEvent, VerbalixError},
    platform::{is_current_caller, OverlaySurface},
    AppRuntime,
};
use std::sync::Arc;
use tauri::{Manager, State, WebviewWindow};

#[tauri::command]
pub(crate) fn dismiss_overlays(runtime: State<'_, Arc<AppRuntime>>) -> Result<(), VerbalixError> {
    runtime.coordinator.dispatch(SelectionEvent::Invalidated)
}

#[tauri::command]
pub(crate) async fn overlay_surface_ready(
    runtime: State<'_, Arc<AppRuntime>>,
    window: WebviewWindow,
    overlay: String,
    generation: uuid::Uuid,
) -> Result<bool, VerbalixError> {
    let surface = OverlaySurface::from_label(&overlay)?;
    if !is_current_caller(window.app_handle(), &window, surface)? {
        return Ok(false);
    }
    runtime.overlay.surface_ready(&overlay, generation).await
}
