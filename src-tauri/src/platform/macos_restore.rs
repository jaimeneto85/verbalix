use super::macos_accessibility::{AXUIElementRef, MacAccessibility, AX_SUCCESS};
use crate::domain::{SelectionSnapshot, VerbalixError};
use core_foundation::{
    base::{CFRelease, CFTypeRef, TCFType},
    string::{CFString, CFStringRef},
};
use std::ffi::c_void;

type AXError = i32;
type AXValueRef = *const c_void;
const AX_VALUE_CF_RANGE: i32 = 4;

#[repr(C)]
struct CFRange {
    location: isize,
    length: isize,
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;
    fn AXValueCreate(value_type: i32, value: *const c_void) -> AXValueRef;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
}

pub fn restore(expected: &SelectionSnapshot, transformed_text: &str) -> Result<(), VerbalixError> {
    let element = MacAccessibility::focused_element()?;
    let mut pid = 0;
    if unsafe { AXUIElementGetPid(element.as_ref(), &mut pid) } != AX_SUCCESS || pid != expected.pid
    {
        return Err(VerbalixError::StaleSelection);
    }
    let value = MacAccessibility::string_attribute(element.as_ref(), "AXValue")?;
    let utf16: Vec<u16> = value.encode_utf16().collect();
    let start =
        usize::try_from(expected.range.location).map_err(|_| VerbalixError::StaleSelection)?;
    let length = transformed_text.encode_utf16().count();
    let end = start
        .checked_add(length)
        .filter(|end| *end <= utf16.len())
        .ok_or(VerbalixError::StaleSelection)?;
    let current =
        String::from_utf16(&utf16[start..end]).map_err(|_| VerbalixError::StaleSelection)?;
    if current != transformed_text {
        return Err(VerbalixError::StaleSelection);
    }
    select_range(element.as_ref(), start, length)?;
    let attribute = CFString::new("AXSelectedText");
    let original = CFString::new(&expected.text);
    let status = unsafe {
        AXUIElementSetAttributeValue(
            element.as_ref(),
            attribute.as_concrete_TypeRef(),
            original.as_CFTypeRef(),
        )
    };
    (status == AX_SUCCESS)
        .then_some(())
        .ok_or(VerbalixError::LocalFailure)
}

fn select_range(
    element: AXUIElementRef,
    location: usize,
    length: usize,
) -> Result<(), VerbalixError> {
    let range = CFRange {
        location: location as isize,
        length: length as isize,
    };
    let value = unsafe { AXValueCreate(AX_VALUE_CF_RANGE, (&range as *const CFRange).cast()) };
    if value.is_null() {
        return Err(VerbalixError::LocalFailure);
    }
    let attribute = CFString::new("AXSelectedTextRange");
    let status =
        unsafe { AXUIElementSetAttributeValue(element, attribute.as_concrete_TypeRef(), value) };
    unsafe { CFRelease(value) };
    (status == AX_SUCCESS)
        .then_some(())
        .ok_or(VerbalixError::LocalFailure)
}
