use crate::{
    application::OverlayPort,
    domain::{Rect, VerbalixError},
};
use serde_json::json;
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

pub struct TauriOverlay {
    app: AppHandle,
}

impl TauriOverlay {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn window(
        &self,
        label: &str,
        width: f64,
        height: f64,
    ) -> Result<WebviewWindow, VerbalixError> {
        if let Some(window) = self.app.get_webview_window(label) {
            return Ok(window);
        }
        WebviewWindowBuilder::new(
            &self.app,
            label,
            WebviewUrl::App(format!("index.html?overlay={label}").into()),
        )
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
    }

    fn place(window: &WebviewWindow, bounds: Rect, width: f64, height: f64) {
        let x = bounds.x + bounds.width / 2.0 - width / 2.0;
        let y = (bounds.y - height - 10.0).max(8.0);
        let _ = window.set_position(LogicalPosition::new(x.max(8.0), y));
    }
}

impl OverlayPort for TauriOverlay {
    fn show_toolbar(&self, bounds: Rect) -> Result<(), VerbalixError> {
        let window = self.window("toolbar", 236.0, 52.0)?;
        Self::place(&window, bounds, 236.0, 52.0);
        window
            .set_focusable(false)
            .map_err(|_| VerbalixError::LocalFailure)?;
        window.show().map_err(|_| VerbalixError::LocalFailure)
    }

    fn show_note(&self, bounds: Rect, text: &str) -> Result<(), VerbalixError> {
        let window = self.window("note", 420.0, 220.0)?;
        Self::place(&window, bounds, 420.0, 220.0);
        window
            .emit("note-result", json!({ "text": text }))
            .map_err(|_| VerbalixError::LocalFailure)?;
        window.show().map_err(|_| VerbalixError::LocalFailure)
    }

    fn hide_all(&self) -> Result<(), VerbalixError> {
        for label in ["toolbar", "note"] {
            if let Some(window) = self.app.get_webview_window(label) {
                window.hide().map_err(|_| VerbalixError::LocalFailure)?;
            }
        }
        Ok(())
    }
}
