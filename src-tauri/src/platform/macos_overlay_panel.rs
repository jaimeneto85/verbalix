use crate::{
    domain::{Rect, VerbalixError},
    platform::overlay_geometry::{
        anchored_origin, ax_to_cocoa, select_screen, AxRect, CocoaPoint, CocoaRect, ScreenFrame,
    },
};
use objc2::{
    msg_send,
    runtime::{AnyClass, AnyObject},
    MainThreadMarker,
};
use objc2_app_kit::NSScreen;
use tauri::WebviewWindow;

pub fn configure(window: &WebviewWindow) -> Result<(), VerbalixError> {
    let object = native_object(window)?;
    let panel_class = AnyClass::get(c"NSPanel").ok_or(VerbalixError::LocalFailure)?;
    let color_class = AnyClass::get(c"NSColor").ok_or(VerbalixError::LocalFailure)?;
    unsafe {
        AnyObject::set_class(object, panel_class);
        let style: usize = msg_send![object, styleMask];
        let clear_color: *mut AnyObject = msg_send![color_class, clearColor];
        if clear_color.is_null() {
            return Err(VerbalixError::LocalFailure);
        }
        let _: () = msg_send![object, setStyleMask: style | (1 << 7)];
        let _: () = msg_send![object, setHidesOnDeactivate: false];
        let _: () = msg_send![object, setBecomesKeyOnlyIfNeeded: true];
        let _: () = msg_send![object, setLevel: 101isize];
        let _: () = msg_send![object, setCollectionBehavior: (1usize << 0) | (1usize << 8)];
        let _: () = msg_send![object, setOpaque: false];
        let _: () = msg_send![object, setBackgroundColor: clear_color];
    }
    Ok(())
}

pub fn place(
    window: &WebviewWindow,
    bounds: Rect,
    width: f64,
    height: f64,
) -> Result<CocoaPoint, VerbalixError> {
    let mtm = MainThreadMarker::new().ok_or(VerbalixError::LocalFailure)?;
    let main_screen = NSScreen::mainScreen(mtm).ok_or(VerbalixError::LocalFailure)?;
    let main_frame = main_screen.frame();
    let main_max_y = main_frame.origin.y + main_frame.size.height;
    let screens: Vec<_> = NSScreen::screens(mtm)
        .iter()
        .map(|screen| ScreenFrame {
            full: CocoaRect(rect(screen.frame())),
            visible: CocoaRect(rect(screen.visibleFrame())),
        })
        .collect();
    let selection = ax_to_cocoa(AxRect(bounds), main_max_y).ok_or(VerbalixError::LocalFailure)?;
    let screen = select_screen(selection, &screens).ok_or(VerbalixError::LocalFailure)?;
    let origin =
        anchored_origin(selection, width, height, screen).ok_or(VerbalixError::LocalFailure)?;
    let object = native_object(window)?;
    let mut native_origin = main_frame.origin;
    native_origin.x = origin.x;
    native_origin.y = origin.y;
    unsafe {
        let _: () = msg_send![object, setFrameOrigin: native_origin];
    }
    Ok(origin)
}

fn native_object(window: &WebviewWindow) -> Result<&AnyObject, VerbalixError> {
    let pointer = window
        .ns_window()
        .map_err(|_| VerbalixError::LocalFailure)?
        .cast::<AnyObject>();
    unsafe { pointer.as_ref() }.ok_or(VerbalixError::LocalFailure)
}

fn rect(frame: objc2_foundation::NSRect) -> Rect {
    Rect {
        x: frame.origin.x,
        y: frame.origin.y,
        width: frame.size.width,
        height: frame.size.height,
    }
}
