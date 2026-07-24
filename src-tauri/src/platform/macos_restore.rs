use super::{
    macos_accessibility::{MacAccessibility, AX_SUCCESS},
    macos_ax::AXUIElementRef,
};
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
    if !expected.writable {
        return Err(VerbalixError::StaleSelection);
    }
    let element = MacAccessibility::focused_element()?;
    let mut pid = 0;
    let pid_status = unsafe { AXUIElementGetPid(element.as_ref(), &mut pid) };
    let own_pid = i32::try_from(std::process::id()).map_err(|_| VerbalixError::StaleSelection)?;
    let role = MacAccessibility::string_attribute(element.as_ref(), "AXRole")?;
    validate_restore_target(
        expected,
        pid_status == AX_SUCCESS,
        pid,
        own_pid,
        &role,
        MacAccessibility::writable(element.as_ref()),
    )?;
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

fn validate_restore_target(
    expected: &SelectionSnapshot,
    pid_available: bool,
    pid: i32,
    own_pid: i32,
    role: &str,
    writable: bool,
) -> Result<(), VerbalixError> {
    if role == "AXSecureTextField" {
        return Err(VerbalixError::ProtectedField);
    }
    if !expected.writable
        || !pid_available
        || pid <= 0
        || pid == own_pid
        || pid != expected.pid
        || role.is_empty()
        || !writable
    {
        return Err(VerbalixError::StaleSelection);
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Rect, TextRange};

    fn snapshot(writable: bool) -> SelectionSnapshot {
        SelectionSnapshot::new(
            42,
            "pid:42".to_owned(),
            "original".to_owned(),
            TextRange {
                location: 3,
                length: 8,
            },
            Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            },
            writable,
        )
    }

    #[test]
    fn marker_snapshot_never_reaches_restore_mutation() {
        assert!(matches!(
            validate_restore_target(&snapshot(false), true, 42, 7, "AXTextArea", true),
            Err(VerbalixError::StaleSelection)
        ));
    }

    #[test]
    fn restore_rejects_self_wrong_pid_secure_and_read_only_targets() {
        let expected = snapshot(true);

        assert!(matches!(
            validate_restore_target(&expected, true, 42, 42, "AXTextArea", true),
            Err(VerbalixError::StaleSelection)
        ));
        assert!(matches!(
            validate_restore_target(&expected, true, 43, 7, "AXTextArea", true),
            Err(VerbalixError::StaleSelection)
        ));
        assert!(matches!(
            validate_restore_target(&expected, true, 42, 7, "AXSecureTextField", true),
            Err(VerbalixError::ProtectedField)
        ));
        assert!(matches!(
            validate_restore_target(&expected, true, 42, 7, "AXTextArea", false),
            Err(VerbalixError::StaleSelection)
        ));
    }

    #[test]
    fn restore_rejects_missing_invalid_and_unidentified_targets() {
        let expected = snapshot(true);

        for (pid_available, pid, role) in [
            (false, 42, "AXTextArea"),
            (true, 0, "AXTextArea"),
            (true, -1, "AXTextArea"),
            (true, 42, ""),
        ] {
            assert!(matches!(
                validate_restore_target(&expected, pid_available, pid, 7, role, true),
                Err(VerbalixError::StaleSelection)
            ));
        }
    }

    #[test]
    fn restore_accepts_the_expected_classic_target() {
        assert!(validate_restore_target(&snapshot(true), true, 42, 7, "AXTextArea", true).is_ok());
    }
}
