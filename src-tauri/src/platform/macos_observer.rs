use super::{
    macos_accessibility::MacAccessibility,
    macos_ax::{AXUIElementRef, AX_SUCCESS},
    macos_element_token::{self, AxElementToken},
};
use core_foundation::{
    base::{CFRelease, TCFType},
    string::{CFString, CFStringRef},
};
use core_foundation_sys::runloop::{
    kCFRunLoopDefaultMode, CFRunLoopAddSource, CFRunLoopGetCurrent, CFRunLoopRemoveSource,
    CFRunLoopRunInMode, CFRunLoopSourceRef,
};
use std::{ffi::c_void, ptr, sync::Arc, thread, time::Duration};

type AXObserverRef = *const c_void;
type AXError = i32;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum AccessibilityEventKind {
    FocusChanged,
    SelectedTextChanged,
    ElementDestroyed,
}

#[derive(Clone)]
pub(super) struct AccessibilityEvent {
    pub(super) kind: AccessibilityEventKind,
    pub(super) target: Option<AxElementToken>,
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;
    fn AXObserverCreate(
        pid: i32,
        callback: extern "C" fn(AXObserverRef, AXUIElementRef, CFStringRef, *mut c_void),
        observer: *mut AXObserverRef,
    ) -> AXError;
    fn AXObserverAddNotification(
        observer: AXObserverRef,
        element: AXUIElementRef,
        notification: CFStringRef,
        context: *mut c_void,
    ) -> AXError;
    fn AXObserverGetRunLoopSource(observer: AXObserverRef) -> CFRunLoopSourceRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
}

extern "C" fn observer_callback(
    _observer: AXObserverRef,
    element: AXUIElementRef,
    notification: CFStringRef,
    context: *mut c_void,
) {
    let Some(callback) =
        (unsafe { (context as *const Arc<dyn Fn(AccessibilityEvent) + Send + Sync>).as_ref() })
    else {
        return;
    };
    if notification.is_null() {
        return;
    }
    let name = unsafe { CFString::wrap_under_get_rule(notification) }.to_string();
    let kind = match name.as_str() {
        "AXFocusedUIElementChanged" => AccessibilityEventKind::FocusChanged,
        "AXSelectedTextChanged" => AccessibilityEventKind::SelectedTextChanged,
        "AXUIElementDestroyed" => AccessibilityEventKind::ElementDestroyed,
        _ => return,
    };
    callback(AccessibilityEvent {
        kind,
        target: macos_element_token::read(element).ok().flatten(),
    });
}

fn publish_focus_change(
    callback: &Arc<dyn Fn(AccessibilityEvent) + Send + Sync>,
    target: Option<AxElementToken>,
) {
    callback(AccessibilityEvent {
        kind: AccessibilityEventKind::FocusChanged,
        target,
    });
}

unsafe fn add_notification(
    observer: AXObserverRef,
    element: AXUIElementRef,
    name: &str,
    context: *mut c_void,
) -> bool {
    let notification = CFString::new(name);
    unsafe {
        AXObserverAddNotification(
            observer,
            element,
            notification.as_concrete_TypeRef(),
            context,
        ) == AX_SUCCESS
    }
}

pub fn start(callback: Arc<dyn Fn(AccessibilityEvent) + Send + Sync>) {
    thread::spawn(move || {
        let context = Box::into_raw(Box::new(callback.clone())).cast::<c_void>();
        let mut previous_target = None;
        loop {
            let element = match MacAccessibility::focused_element() {
                Ok(element) => element,
                Err(_) => {
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
            };
            let current_target = macos_element_token::read(element.as_ref()).ok().flatten();
            if previous_target.is_some() && previous_target != current_target {
                publish_focus_change(&callback, current_target.clone());
            }
            previous_target = current_target;
            let mut pid = 0;
            if unsafe { AXUIElementGetPid(element.as_ref(), &mut pid) } != AX_SUCCESS {
                continue;
            }
            let application = unsafe { AXUIElementCreateApplication(pid) };
            if application.is_null() {
                continue;
            }
            let mut observer: AXObserverRef = ptr::null();
            if unsafe { AXObserverCreate(pid, observer_callback, &mut observer) } != AX_SUCCESS
                || observer.is_null()
            {
                unsafe { CFRelease(application) };
                continue;
            }
            let registered = unsafe {
                add_notification(observer, element.as_ref(), "AXSelectedTextChanged", context)
                    && add_notification(observer, element.as_ref(), "AXUIElementDestroyed", context)
                    && add_notification(observer, application, "AXFocusedUIElementChanged", context)
            };
            if !registered {
                unsafe {
                    CFRelease(observer);
                    CFRelease(application);
                }
                continue;
            }
            let source = unsafe { AXObserverGetRunLoopSource(observer) };
            let run_loop = unsafe { CFRunLoopGetCurrent() };
            unsafe {
                CFRunLoopAddSource(run_loop, source, kCFRunLoopDefaultMode);
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.75, 1);
                CFRunLoopRemoveSource(run_loop, source, kCFRunLoopDefaultMode);
                CFRelease(observer);
                CFRelease(application);
            }
        }
    });
}
