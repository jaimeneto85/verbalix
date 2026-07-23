use crate::{
    domain::{Rect, VerbalixError},
    platform::note_result::NoteResultPayload,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

#[derive(Clone, Debug, PartialEq)]
pub enum OverlayCommand {
    ShowToolbar(Rect),
    ShowResult(Rect, NoteResultPayload),
    HideAll,
}

pub trait OverlayDispatcher: Send + Sync {
    fn dispatch(&self, command: OverlayCommand) -> Result<(), VerbalixError>;
}

pub struct MainThreadOverlayDispatcher {
    app: AppHandle,
    execution_failure: Arc<ExecutionFailure>,
}

#[derive(Default)]
struct ExecutionFailure {
    pending: AtomicBool,
}

impl ExecutionFailure {
    fn record(&self) {
        self.pending.store(true, Ordering::Release);
    }

    fn take(&self) -> Result<(), VerbalixError> {
        if self.pending.swap(false, Ordering::AcqRel) {
            return Err(VerbalixError::LocalFailure);
        }
        Ok(())
    }
}

impl MainThreadOverlayDispatcher {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            execution_failure: Arc::new(ExecutionFailure::default()),
        }
    }
}

impl OverlayDispatcher for MainThreadOverlayDispatcher {
    fn dispatch(&self, command: OverlayCommand) -> Result<(), VerbalixError> {
        self.execution_failure.take()?;
        let app = self.app.clone();
        let execution_failure = self.execution_failure.clone();
        self.app
            .run_on_main_thread(move || {
                if execute_command(&app, command).is_err() {
                    execution_failure.record();
                }
            })
            .map_err(|_| VerbalixError::LocalFailure)
    }
}

fn execute_command(app: &AppHandle, command: OverlayCommand) -> Result<(), VerbalixError> {
    match command {
        OverlayCommand::ShowToolbar(bounds) => {
            let window = window(app, "toolbar", 236.0, 52.0)?;
            place(app, &window, bounds, 236.0, 52.0);
            window
                .set_focusable(false)
                .map_err(|_| VerbalixError::LocalFailure)?;
            window.show().map_err(|_| VerbalixError::LocalFailure)
        }
        OverlayCommand::ShowResult(bounds, payload) => {
            let window = window(app, "note", 420.0, 220.0)?;
            place(app, &window, bounds, 420.0, 220.0);
            window
                .emit("note-result", payload)
                .map_err(|_| VerbalixError::LocalFailure)?;
            window.show().map_err(|_| VerbalixError::LocalFailure)
        }
        OverlayCommand::HideAll => {
            for label in ["toolbar", "note"] {
                if let Some(window) = app.get_webview_window(label) {
                    window.hide().map_err(|_| VerbalixError::LocalFailure)?;
                }
            }
            Ok(())
        }
    }
}

fn window(
    app: &AppHandle,
    label: &str,
    width: f64,
    height: f64,
) -> Result<WebviewWindow, VerbalixError> {
    if let Some(window) = app.get_webview_window(label) {
        return Ok(window);
    }
    let window = WebviewWindowBuilder::new(
        app,
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

fn place(app: &AppHandle, window: &WebviewWindow, bounds: Rect, width: f64, height: f64) {
    let frame = visible_frames().into_iter().find(|frame| {
        bounds.x >= frame.x
            && bounds.x <= frame.x + frame.width
            && bounds.y >= frame.y
            && bounds.y <= frame.y + frame.height
    });
    let frame = frame.or_else(|| {
        app.available_monitors().ok().and_then(|monitors| {
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
        })
    });
    let (x, y) = anchored_origin(bounds, width, height, frame);
    let _ = window.set_position(LogicalPosition::new(x, y));
}

#[cfg(target_os = "macos")]
fn visible_frames() -> Vec<Rect> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;
    let Some(mtm) = MainThreadMarker::new() else {
        return Vec::new();
    };
    let primary_height = NSScreen::mainScreen(mtm)
        .map(|screen| screen.frame().size.height)
        .unwrap_or_default();
    NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let visible = screen.visibleFrame();
            Rect {
                x: visible.origin.x,
                y: primary_height - visible.origin.y - visible.size.height,
                width: visible.size.width,
                height: visible.size.height,
            }
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
fn visible_frames() -> Vec<Rect> {
    Vec::new()
}

pub fn anchored_origin(
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
    fn asynchronous_execution_failure_is_reported_once_without_waiting() {
        let failure = ExecutionFailure::default();

        failure.record();

        assert!(matches!(failure.take(), Err(VerbalixError::LocalFailure)));
        assert!(failure.take().is_ok());
    }
}
