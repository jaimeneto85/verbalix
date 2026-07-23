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

    fn window(&self, label: &str, width: f64, height: f64) -> Result<WebviewWindow, VerbalixError> {
        if let Some(window) = self.app.get_webview_window(label) {
            return Ok(window);
        }
        let window = WebviewWindowBuilder::new(
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
        .map_err(|_| VerbalixError::LocalFailure)?;
        #[cfg(target_os = "macos")]
        configure_nonactivating_panel(&window)?;
        Ok(window)
    }

    fn place(&self, window: &WebviewWindow, bounds: Rect, width: f64, height: f64) {
        let frame = self.app.available_monitors().ok().and_then(|monitors| {
            monitors.into_iter().find_map(|monitor| {
                let scale = monitor.scale_factor();
                let position = monitor.position();
                let size = monitor.size();
                let frame = Rect {
                    x: f64::from(position.x) / scale,
                    y: f64::from(position.y) / scale,
                    width: f64::from(size.width) / scale,
                    height: f64::from(size.height) / scale,
                };
                let contained = bounds.x >= frame.x
                    && bounds.x <= frame.x + frame.width
                    && bounds.y >= frame.y
                    && bounds.y <= frame.y + frame.height;
                contained.then_some(frame)
            })
        });
        let (x, y) = anchored_origin(bounds, width, height, frame);
        let _ = window.set_position(LogicalPosition::new(x, y));
    }

    fn show_result(&self, bounds: Rect, payload: serde_json::Value) -> Result<(), VerbalixError> {
        let window = self.window("note", 420.0, 220.0)?;
        self.place(&window, bounds, 420.0, 220.0);
        window
            .emit("note-result", payload)
            .map_err(|_| VerbalixError::LocalFailure)?;
        window.show().map_err(|_| VerbalixError::LocalFailure)
    }
}

fn anchored_origin(
    bounds: Rect,
    width: f64,
    height: f64,
    visible_frame: Option<Rect>,
) -> (f64, f64) {
    let x = bounds.x + bounds.width / 2.0 - width / 2.0;
    let y = bounds.y - height - 10.0;
    if let Some(frame) = visible_frame {
        return (
            x.clamp(frame.x + 8.0, frame.x + frame.width - width - 8.0),
            y.clamp(frame.y + 8.0, frame.y + frame.height - height - 8.0),
        );
    }
    (x.max(8.0), y.max(8.0))
}

#[cfg(target_os = "macos")]
fn configure_nonactivating_panel(window: &WebviewWindow) -> Result<(), VerbalixError> {
    use objc2::{
        msg_send,
        runtime::{AnyClass, AnyObject},
    };
    let pointer = window
        .ns_window()
        .map_err(|_| VerbalixError::LocalFailure)?
        .cast::<AnyObject>();
    let object = unsafe { pointer.as_ref() }.ok_or(VerbalixError::LocalFailure)?;
    let panel_class = AnyClass::get(c"NSPanel").ok_or(VerbalixError::LocalFailure)?;
    unsafe {
        AnyObject::set_class(object, panel_class);
        let style: usize = msg_send![object, styleMask];
        let _: () = msg_send![object, setStyleMask: style | (1 << 7)];
        let _: () = msg_send![object, setHidesOnDeactivate: false];
        let _: () = msg_send![object, setBecomesKeyOnlyIfNeeded: true];
        let _: () = msg_send![object, setLevel: 101isize];
        let _: () = msg_send![object, setCollectionBehavior: (1usize << 0) | (1usize << 8)];
    }
    Ok(())
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
            None,
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
            None,
        );
        assert_eq!(origin, (8.0, 8.0));
    }
}

impl OverlayPort for TauriOverlay {
    fn show_toolbar(&self, bounds: Rect) -> Result<(), VerbalixError> {
        let window = self.window("toolbar", 236.0, 52.0)?;
        self.place(&window, bounds, 236.0, 52.0);
        window
            .set_focusable(false)
            .map_err(|_| VerbalixError::LocalFailure)?;
        window.show().map_err(|_| VerbalixError::LocalFailure)
    }

    fn show_note(&self, bounds: Rect, text: &str) -> Result<(), VerbalixError> {
        self.show_result(bounds, json!({ "mode": "result", "text": text }))
    }

    fn show_preview(
        &self,
        bounds: Rect,
        request_id: uuid::Uuid,
        text: &str,
    ) -> Result<(), VerbalixError> {
        self.show_result(
            bounds,
            json!({ "mode": "preview", "requestId": request_id, "text": text }),
        )
    }

    fn show_undo(&self, bounds: Rect, text: &str) -> Result<(), VerbalixError> {
        self.show_result(bounds, json!({ "mode": "undo", "text": text }))
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
