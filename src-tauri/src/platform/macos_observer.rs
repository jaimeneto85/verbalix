use super::{
    macos_accessibility::MacAccessibility,
    macos_ax::{AXUIElementRef, AX_SUCCESS},
    macos_element_token::{self, AxElementToken},
};
use core_foundation::{
    base::{CFEqual, CFRelease, TCFType},
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

struct ObserverContext {
    callback: Arc<dyn Fn(AccessibilityEvent) + Send + Sync>,
    has_pending_self_notification: Arc<dyn Fn() -> bool + Send + Sync>,
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
    let Some(context) = (unsafe { (context as *const ObserverContext).as_ref() }) else {
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
    publish_notification(context, kind, || {
        macos_element_token::read(element).ok().flatten()
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

fn publish_notification(
    context: &ObserverContext,
    kind: AccessibilityEventKind,
    read_target: impl FnOnce() -> Option<AxElementToken>,
) {
    let target = match kind {
        AccessibilityEventKind::SelectedTextChanged
            if (context.has_pending_self_notification)() =>
        {
            read_target()
        }
        AccessibilityEventKind::FocusChanged
        | AccessibilityEventKind::SelectedTextChanged
        | AccessibilityEventKind::ElementDestroyed => None,
    };
    (context.callback)(AccessibilityEvent { kind, target });
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

pub fn start(
    callback: Arc<dyn Fn(AccessibilityEvent) + Send + Sync>,
    has_pending_self_notification: Arc<dyn Fn() -> bool + Send + Sync>,
) {
    thread::spawn(move || {
        let context = Box::into_raw(Box::new(ObserverContext {
            callback: callback.clone(),
            has_pending_self_notification,
        }))
        .cast::<c_void>();
        let mut previous_element: Option<super::macos_ax::OwnedAxElement> = None;
        loop {
            let element = match MacAccessibility::focused_element() {
                Ok(element) => element,
                Err(_) => {
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
            };
            if previous_element.as_ref().is_some_and(|previous| unsafe {
                CFEqual(previous.as_ref().cast(), element.as_ref().cast()) == 0
            }) {
                publish_focus_change(&callback, None);
            }
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
            previous_element = Some(element);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn context(pending: bool, events: Arc<AtomicUsize>) -> ObserverContext {
        ObserverContext {
            callback: Arc::new(move |_| {
                events.fetch_add(1, Ordering::SeqCst);
            }),
            has_pending_self_notification: Arc::new(move || pending),
        }
    }

    #[test]
    fn focus_destroy_and_unexpected_selection_publish_without_token_reads() {
        for (kind, pending) in [
            (AccessibilityEventKind::FocusChanged, true),
            (AccessibilityEventKind::ElementDestroyed, true),
            (AccessibilityEventKind::SelectedTextChanged, false),
        ] {
            let events = Arc::new(AtomicUsize::new(0));
            let reads = AtomicUsize::new(0);
            publish_notification(&context(pending, events.clone()), kind, || {
                reads.fetch_add(1, Ordering::SeqCst);
                AxElementToken::new(42, "private")
            });
            assert_eq!(reads.load(Ordering::SeqCst), 0);
            assert_eq!(events.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn expected_selection_is_the_only_notification_that_reads_a_token() {
        let events = Arc::new(AtomicUsize::new(0));
        let reads = AtomicUsize::new(0);
        publish_notification(
            &context(true, events.clone()),
            AccessibilityEventKind::SelectedTextChanged,
            || {
                reads.fetch_add(1, Ordering::SeqCst);
                AxElementToken::new(42, "private")
            },
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        assert_eq!(events.load(Ordering::SeqCst), 1);
    }
}
