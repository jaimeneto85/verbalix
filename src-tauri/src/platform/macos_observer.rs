use super::{
    macos_accessibility::MacAccessibility,
    macos_ax::{AXUIElementRef, AX_SUCCESS},
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
}

extern "C" fn observer_callback(
    _observer: AXObserverRef,
    _element: AXUIElementRef,
    _notification: CFStringRef,
    context: *mut c_void,
) {
    if let Some(callback) = unsafe { (context as *const Arc<dyn Fn() + Send + Sync>).as_ref() } {
        callback();
    }
}

pub fn start(callback: Arc<dyn Fn() + Send + Sync>) {
    thread::spawn(move || {
        let context = Box::into_raw(Box::new(callback)).cast::<c_void>();
        loop {
            let element = match MacAccessibility::focused_element() {
                Ok(element) => element,
                Err(_) => {
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
            };
            let mut pid = 0;
            if unsafe { AXUIElementGetPid(element.as_ref(), &mut pid) } != AX_SUCCESS {
                continue;
            }
            let mut observer: AXObserverRef = ptr::null();
            if unsafe { AXObserverCreate(pid, observer_callback, &mut observer) } != AX_SUCCESS
                || observer.is_null()
            {
                continue;
            }
            for name in ["AXSelectedTextChanged", "AXUIElementDestroyed"] {
                let notification = CFString::new(name);
                unsafe {
                    AXObserverAddNotification(
                        observer,
                        element.as_ref(),
                        notification.as_concrete_TypeRef(),
                        context,
                    );
                }
            }
            let source = unsafe { AXObserverGetRunLoopSource(observer) };
            let run_loop = unsafe { CFRunLoopGetCurrent() };
            unsafe {
                CFRunLoopAddSource(run_loop, source, kCFRunLoopDefaultMode);
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.75, 1);
                CFRunLoopRemoveSource(run_loop, source, kCFRunLoopDefaultMode);
                CFRelease(observer);
            }
        }
    });
}
