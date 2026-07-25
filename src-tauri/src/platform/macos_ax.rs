use super::macos_focus::{AxCategory, AxFailure, AxStage, ExtractionOrigin};
use core_foundation::{
    base::{CFGetTypeID, CFRelease, CFTypeRef, TCFType},
    boolean::CFBoolean,
    dictionary::CFDictionary,
    string::{CFString, CFStringGetTypeID, CFStringRef},
};
use core_foundation_sys::{base::Boolean, dictionary::CFDictionaryRef};
use std::{
    ffi::c_void,
    mem,
    ptr::{self, NonNull},
};

pub(super) type AXUIElementRef = *const c_void;
type AXError = i32;

pub(super) const AX_SUCCESS: AXError = 0;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> Boolean;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> Boolean;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementGetTypeID() -> usize;
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
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;
}

pub(super) struct OwnedAxElement(NonNull<c_void>);

impl OwnedAxElement {
    fn from_created(element: AXUIElementRef, stage: AxStage) -> Result<Self, AxFailure> {
        NonNull::new(element.cast_mut())
            .map(Self)
            .ok_or_else(|| AxFailure::new(stage, AxCategory::NullValue))
    }

    pub(super) fn as_ref(&self) -> AXUIElementRef {
        self.0.as_ptr()
    }
}

impl Drop for OwnedAxElement {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0.as_ptr()) }
    }
}

pub(super) struct OwnedCfValue(NonNull<c_void>);

impl OwnedCfValue {
    pub(super) fn from_created(value: CFTypeRef, stage: AxStage) -> Result<Self, AxFailure> {
        NonNull::new(value.cast_mut())
            .map(Self)
            .ok_or_else(|| AxFailure::new(stage, AxCategory::NullValue))
    }

    pub(super) fn as_ref(&self) -> CFTypeRef {
        self.0.as_ptr()
    }

    fn into_ax_element(
        self,
        stage: AxStage,
        origin: ExtractionOrigin,
    ) -> Result<OwnedAxElement, AxFailure> {
        if unsafe { CFGetTypeID(self.as_ref()) != AXUIElementGetTypeID() } {
            crate::diagnostics::ax_resolution(stage, origin, AxCategory::UnexpectedType);
            return Err(AxFailure::new(stage, AxCategory::UnexpectedType));
        }
        let element = OwnedAxElement(self.0);
        mem::forget(self);
        Ok(element)
    }
}

impl Drop for OwnedCfValue {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0.as_ptr()) }
    }
}

pub(super) fn trusted(prompt: bool) -> bool {
    unsafe {
        if !prompt {
            return AXIsProcessTrusted() != 0;
        }
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0
    }
}

pub(super) fn focused_element() -> Result<OwnedAxElement, AxFailure> {
    let origin = ExtractionOrigin::SelectedText;
    let system = unsafe { AXUIElementCreateSystemWide() };
    let system = OwnedAxElement::from_created(system, AxStage::SystemWideFocusedElement)?;
    attribute(
        system.as_ref(),
        "AXFocusedUIElement",
        AxStage::SystemWideFocusedElement,
        origin,
    )?
    .into_ax_element(AxStage::SystemWideFocusedElement, origin)
}

pub(super) fn attribute(
    element: AXUIElementRef,
    name: &str,
    stage: AxStage,
    origin: ExtractionOrigin,
) -> Result<OwnedCfValue, AxFailure> {
    let attribute = CFString::new(name);
    let mut value: CFTypeRef = ptr::null();
    let status = unsafe {
        AXUIElementCopyAttributeValue(element, attribute.as_concrete_TypeRef(), &mut value)
    };
    owned_copy_result(value, status, stage, origin)
}

pub(super) fn parameterized_attribute(
    element: AXUIElementRef,
    name: &str,
    parameter: CFTypeRef,
    stage: AxStage,
    origin: ExtractionOrigin,
) -> Result<OwnedCfValue, AxFailure> {
    let attribute = CFString::new(name);
    let mut value: CFTypeRef = ptr::null();
    let status = unsafe {
        AXUIElementCopyParameterizedAttributeValue(
            element,
            attribute.as_concrete_TypeRef(),
            parameter,
            &mut value,
        )
    };
    owned_copy_result(value, status, stage, origin)
}

fn owned_copy_result(
    value: CFTypeRef,
    status: AXError,
    stage: AxStage,
    origin: ExtractionOrigin,
) -> Result<OwnedCfValue, AxFailure> {
    let category = AxCategory::from_status(status);
    if status != AX_SUCCESS {
        if let Some(value) = NonNull::new(value.cast_mut()) {
            unsafe { CFRelease(value.as_ptr()) };
        }
        crate::diagnostics::ax_resolution(stage, origin, category);
        return Err(AxFailure::new(stage, category));
    }
    let value = OwnedCfValue::from_created(value, stage).inspect_err(|failure| {
        crate::diagnostics::ax_resolution(stage, origin, failure.category);
    })?;
    crate::diagnostics::ax_resolution(stage, origin, AxCategory::Success);
    Ok(value)
}

pub(super) fn string_value(
    value: &OwnedCfValue,
    stage: AxStage,
    origin: ExtractionOrigin,
) -> Result<String, AxFailure> {
    if unsafe { CFGetTypeID(value.as_ref()) != CFStringGetTypeID() } {
        crate::diagnostics::ax_resolution(stage, origin, AxCategory::UnexpectedType);
        return Err(AxFailure::new(stage, AxCategory::UnexpectedType));
    }
    let value = unsafe { CFString::wrap_under_get_rule(value.as_ref().cast()) };
    Ok(value.to_string())
}

pub(super) fn string_attribute(
    element: AXUIElementRef,
    name: &str,
    stage: AxStage,
    origin: ExtractionOrigin,
) -> Result<String, AxFailure> {
    let value = attribute(element, name, stage, origin)?;
    string_value(&value, stage, origin)
}

pub(super) fn optional_string_attribute(
    element: AXUIElementRef,
    name: &str,
    stage: AxStage,
    origin: ExtractionOrigin,
) -> Result<Option<String>, AxFailure> {
    match string_attribute(element, name, stage, origin) {
        Ok(value) => Ok(Some(value)),
        Err(failure)
            if matches!(
                failure.category,
                AxCategory::NoValue | AxCategory::AttributeUnsupported
            ) =>
        {
            Ok(None)
        }
        Err(failure) => Err(failure),
    }
}

pub(super) fn pid(element: AXUIElementRef) -> Result<i32, AxFailure> {
    let mut pid = 0;
    let status = unsafe { AXUIElementGetPid(element, &mut pid) };
    if status == AX_SUCCESS && pid > 0 {
        Ok(pid)
    } else {
        let category = if status == AX_SUCCESS {
            AxCategory::InvalidPid
        } else {
            AxCategory::from_status(status)
        };
        Err(AxFailure::new(AxStage::Pid, category))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AxWriteResult {
    Confirmed,
    Rejected(AxCategory),
    Indeterminate(AxCategory),
}

pub(super) struct PreparedSelectedTextWrite {
    element: AXUIElementRef,
    attribute: CFString,
    value: CFString,
}

pub(super) fn prepare_selected_text_write(
    element: AXUIElementRef,
    text: &str,
) -> PreparedSelectedTextWrite {
    PreparedSelectedTextWrite {
        element,
        attribute: CFString::new("AXSelectedText"),
        value: CFString::new(text),
    }
}

pub(super) fn set_prepared_selected_text(write: PreparedSelectedTextWrite) -> AxWriteResult {
    let status = unsafe {
        AXUIElementSetAttributeValue(
            write.element,
            write.attribute.as_concrete_TypeRef(),
            write.value.as_CFTypeRef(),
        )
    };
    classify_write_status(status)
}

fn classify_write_status(status: AXError) -> AxWriteResult {
    let category = AxCategory::from_status(status);
    let result = if status == AX_SUCCESS {
        AxWriteResult::Confirmed
    } else if matches!(category, AxCategory::CannotComplete | AxCategory::Failure) {
        AxWriteResult::Indeterminate(category)
    } else {
        AxWriteResult::Rejected(category)
    };
    crate::diagnostics::ax_resolution(
        AxStage::SelectedTextWrite,
        ExtractionOrigin::SelectedText,
        category,
    );
    result
}

#[cfg(test)]
mod write_tests {
    use super::*;

    #[test]
    fn write_status_distinguishes_confirmed_rejected_and_indeterminate() {
        assert_eq!(classify_write_status(AX_SUCCESS), AxWriteResult::Confirmed);
        assert_eq!(
            classify_write_status(-25205),
            AxWriteResult::Rejected(AxCategory::AttributeUnsupported)
        );
        for status in [-25200, -25204] {
            assert!(matches!(
                classify_write_status(status),
                AxWriteResult::Indeterminate(_)
            ));
        }
    }
}
