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
        let (x, y) = anchored_origin(bounds, width, height);
        let _ = window.set_position(LogicalPosition::new(x, y));
    }
}

fn anchored_origin(bounds: Rect, width: f64, height: f64) -> (f64, f64) {
    let x = bounds.x + bounds.width / 2.0 - width / 2.0;
    let y = bounds.y - height - 10.0;
    (x.max(8.0), y.max(8.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_toolbar_above_selection() {
        let origin = anchored_origin(
            Rect {
                x: 200.0,
                y: 300.0,
                width: 100.0,
                height: 20.0,
            },
            236.0,
            52.0,
        );
        assert_eq!(origin, (132.0, 238.0));
    }

    #[test]
    fn clamps_overlay_to_top_and_left_safe_margin() {
        let origin = anchored_origin(
            Rect {
                x: 0.0,
                y: 4.0,
                width: 1.0,
                height: 1.0,
            },
            420.0,
            220.0,
        );
        assert_eq!(origin, (8.0, 8.0));
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
