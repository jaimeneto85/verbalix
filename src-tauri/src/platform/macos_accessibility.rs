use crate::{
    application::SelectionPort,
    domain::{Rect, SelectionSnapshot, TextRange, VerbalixError},
};
use core_foundation::{
    base::{CFGetTypeID, CFRelease, CFTypeRef, TCFType},
    boolean::CFBoolean,
    dictionary::CFDictionary,
    string::{CFString, CFStringGetTypeID, CFStringRef},
};
use core_foundation_sys::{base::Boolean, dictionary::CFDictionaryRef};
use std::{
    ffi::c_void,
    ptr::{self, NonNull},
};

type AXUIElementRef = *const c_void;
type AXValueRef = *const c_void;
type AXError = i32;

const AX_SUCCESS: AXError = 0;
const AX_VALUE_CF_RANGE: i32 = 4;
const AX_VALUE_CG_RECT: i32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CFRange {
    location: isize,
    length: isize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> Boolean;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> Boolean;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementIsAttributeSettable(
        element: AXUIElementRef,
        attribute: CFStringRef,
        settable: *mut Boolean,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;
    fn AXValueCreate(value_type: i32, value: *const c_void) -> AXValueRef;
    fn AXValueGetValue(value: AXValueRef, value_type: i32, output: *mut c_void) -> Boolean;
}

struct OwnedAxElement(NonNull<c_void>);

impl OwnedAxElement {
    fn as_ref(&self) -> AXUIElementRef {
        self.0.as_ptr()
    }
}

impl Drop for OwnedAxElement {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0.as_ptr()) }
    }
}

pub struct MacAccessibility;

impl MacAccessibility {
    pub fn new() -> Self {
        Self
    }

    fn attribute(element: AXUIElementRef, name: &str) -> Result<CFTypeRef, VerbalixError> {
        let attribute = CFString::new(name);
        let mut value: CFTypeRef = ptr::null();
        let status = unsafe {
            AXUIElementCopyAttributeValue(element, attribute.as_concrete_TypeRef(), &mut value)
        };
        if status != AX_SUCCESS || value.is_null() {
            return Err(VerbalixError::SelectionUnavailable);
        }
        Ok(value)
    }

    fn focused_element() -> Result<OwnedAxElement, VerbalixError> {
        let system = unsafe { AXUIElementCreateSystemWide() };
        let system = NonNull::new(system.cast_mut()).ok_or(VerbalixError::SelectionUnavailable)?;
        let focused = Self::attribute(system.as_ptr(), "AXFocusedUIElement");
        unsafe { CFRelease(system.as_ptr()) };
        let focused = focused?;
        NonNull::new(focused.cast_mut())
            .map(OwnedAxElement)
            .ok_or(VerbalixError::SelectionUnavailable)
    }

    fn string_attribute(
        element: AXUIElementRef,
        name: &str,
    ) -> Result<String, VerbalixError> {
        let value = Self::attribute(element, name)?;
        let is_string = unsafe { CFGetTypeID(value) == CFStringGetTypeID() };
        if !is_string {
            unsafe { CFRelease(value) };
            return Err(VerbalixError::SelectionUnavailable);
        }
        let value = unsafe { CFString::wrap_under_create_rule(value.cast()) };
        Ok(value.to_string())
    }

    fn selected_range(element: AXUIElementRef) -> Result<CFRange, VerbalixError> {
        let value = Self::attribute(element, "AXSelectedTextRange")?;
        let mut range = CFRange::default();
        let success =
            unsafe { AXValueGetValue(value, AX_VALUE_CF_RANGE, (&mut range as *mut CFRange).cast()) };
        unsafe { CFRelease(value) };
        if success == 0 || range.length <= 0 {
            return Err(VerbalixError::SelectionUnavailable);
        }
        Ok(range)
    }

    fn bounds(element: AXUIElementRef, range: CFRange) -> Rect {
        let range_value =
            unsafe { AXValueCreate(AX_VALUE_CF_RANGE, (&range as *const CFRange).cast()) };
        if range_value.is_null() {
            return Rect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            };
        }
        let attribute = CFString::new("AXBoundsForRange");
        let mut value: CFTypeRef = ptr::null();
        let status = unsafe {
            AXUIElementCopyParameterizedAttributeValue(
                element,
                attribute.as_concrete_TypeRef(),
                range_value,
                &mut value,
            )
        };
        unsafe { CFRelease(range_value) };
        let mut bounds = CGRect::default();
        if status == AX_SUCCESS && !value.is_null() {
            unsafe {
                AXValueGetValue(
                    value,
                    AX_VALUE_CG_RECT,
                    (&mut bounds as *mut CGRect).cast(),
                );
                CFRelease(value);
            }
        }
        Rect {
            x: bounds.origin.x,
            y: bounds.origin.y,
            width: bounds.size.width.max(1.0),
            height: bounds.size.height.max(1.0),
        }
    }

    fn writable(element: AXUIElementRef) -> bool {
        let attribute = CFString::new("AXSelectedText");
        let mut settable: Boolean = 0;
        unsafe {
            AXUIElementIsAttributeSettable(
                element,
                attribute.as_concrete_TypeRef(),
                &mut settable,
            ) == AX_SUCCESS
                && settable != 0
        }
    }
}

impl SelectionPort for MacAccessibility {
    fn permission_granted(&self, prompt: bool) -> bool {
        unsafe {
            if !prompt {
                return AXIsProcessTrusted() != 0;
            }
            let prompt_key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let options = CFDictionary::from_CFType_pairs(&[(prompt_key, CFBoolean::true_value())]);
            AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0
        }
    }

    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        if !self.permission_granted(false) {
            return Err(VerbalixError::PermissionDenied);
        }
        let element = Self::focused_element()?;
        let role = Self::string_attribute(element.as_ref(), "AXRole").unwrap_or_default();
        if role == "AXSecureTextField" {
            return Err(VerbalixError::ProtectedField);
        }
        let text = Self::string_attribute(element.as_ref(), "AXSelectedText")?;
        if text.trim().is_empty() {
            return Err(VerbalixError::SelectionUnavailable);
        }
        if text.chars().count() > 12_000 {
            return Err(VerbalixError::TextTooLong);
        }
        let range = Self::selected_range(element.as_ref())?;
        let mut pid = 0;
        if unsafe { AXUIElementGetPid(element.as_ref(), &mut pid) } != AX_SUCCESS {
            return Err(VerbalixError::SelectionUnavailable);
        }
        Ok(SelectionSnapshot::new(
            pid,
            format!("pid:{pid}"),
            text,
            TextRange {
                location: range.location as i64,
                length: range.length as i64,
            },
            Self::bounds(element.as_ref(), range),
            Self::writable(element.as_ref()),
        ))
    }

    fn replace(&self, expected: &SelectionSnapshot, text: &str) -> Result<(), VerbalixError> {
        let current = self.capture()?;
        if !current.same_target(expected) || !current.writable {
            return Err(VerbalixError::StaleSelection);
        }
        let element = Self::focused_element()?;
        let value = CFString::new(text);
        let attribute = CFString::new("AXSelectedText");
        let status = unsafe {
            AXUIElementSetAttributeValue(
                element.as_ref(),
                attribute.as_concrete_TypeRef(),
                value.as_CFTypeRef(),
            )
        };
        if status == AX_SUCCESS {
            Ok(())
        } else {
            Err(VerbalixError::LocalFailure)
        }
    }
}
