#[cfg(target_os = "macos")]
use crate::platform::macos_overlay_panel;
use crate::{
    diagnostics,
    domain::{Rect, VerbalixError},
    platform::note_result::NoteResultPayload,
};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
#[cfg(not(target_os = "macos"))]
use tauri::LogicalPosition;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

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
    next_sequence: AtomicU64,
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
            next_sequence: AtomicU64::new(1),
        }
    }
}

impl OverlayDispatcher for MainThreadOverlayDispatcher {
    fn dispatch(&self, command: OverlayCommand) -> Result<(), VerbalixError> {
        self.execution_failure.take()?;
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        let label = command.label();
        diagnostics::overlay("scheduled", label, sequence);
        let app = self.app.clone();
        let execution_failure = self.execution_failure.clone();
        self.app
            .run_on_main_thread(move || {
                diagnostics::overlay("executing", label, sequence);
                if execute_command(&app, command, sequence).is_err() {
                    diagnostics::overlay("failed", label, sequence);
                    execution_failure.record();
                }
            })
            .map_err(|_| VerbalixError::LocalFailure)
    }
}

impl OverlayCommand {
    fn label(&self) -> &'static str {
        match self {
            Self::ShowToolbar(_) => "toolbar",
            Self::ShowResult(_, _) => "note",
            Self::HideAll => "all",
        }
    }
}

fn execute_command(
    app: &AppHandle,
    command: OverlayCommand,
    sequence: u64,
) -> Result<(), VerbalixError> {
    match command {
        OverlayCommand::ShowToolbar(bounds) => {
            let window = window(app, "toolbar", 236.0, 52.0, sequence)?;
            place(app, &window, bounds, 236.0, 52.0, "toolbar", sequence)?;
            show_and_confirm(&window, "toolbar", sequence)
        }
        OverlayCommand::ShowResult(bounds, payload) => {
            let window = window(app, "note", 420.0, 220.0, sequence)?;
            place(app, &window, bounds, 420.0, 220.0, "note", sequence)?;
            window
                .emit("note-result", payload)
                .map_err(|_| VerbalixError::LocalFailure)?;
            diagnostics::overlay("emitted", "note", sequence);
            show_and_confirm(&window, "note", sequence)
        }
        OverlayCommand::HideAll => {
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
    }
}

fn window(
    app: &AppHandle,
    label: &str,
    width: f64,
    height: f64,
    sequence: u64,
) -> Result<WebviewWindow, VerbalixError> {
    if let Some(window) = app.get_webview_window(label) {
        diagnostics::overlay("window_reused", label, sequence);
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
    macos_overlay_panel::configure(&window)?;
    diagnostics::overlay("window_created", label, sequence);
    Ok(window)
}

#[cfg(target_os = "macos")]
fn place(
    _app: &AppHandle,
    window: &WebviewWindow,
    bounds: Rect,
    width: f64,
    height: f64,
    label: &str,
    sequence: u64,
) -> Result<(), VerbalixError> {
    let origin = macos_overlay_panel::place(window, bounds, width, height)?;
    diagnostics::overlay_position(label, sequence, origin.x, origin.y);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn place(
    app: &AppHandle,
    window: &WebviewWindow,
    bounds: Rect,
    width: f64,
    height: f64,
    label: &str,
    sequence: u64,
) -> Result<(), VerbalixError> {
    let frame = app.available_monitors().ok().and_then(|monitors| {
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
            contains(frame, bounds).then_some(frame)
        })
    });
    let (x, y) = legacy_anchored_origin(bounds, width, height, frame);
    window
        .set_position(LogicalPosition::new(x, y))
        .map_err(|_| VerbalixError::LocalFailure)?;
    diagnostics::overlay_position(label, sequence, x, y);
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

#[cfg(not(target_os = "macos"))]
fn contains(frame: Rect, bounds: Rect) -> bool {
    bounds.x >= frame.x
        && bounds.x <= frame.x + frame.width
        && bounds.y >= frame.y
        && bounds.y <= frame.y + frame.height
}

#[cfg(not(target_os = "macos"))]
fn legacy_anchored_origin(
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
