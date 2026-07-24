use super::{
    macos_ax::{AXUIElementRef, AX_SUCCESS},
    macos_focus::{AxCategory, AxFailure, AxStage, ExtractionOrigin},
};
use core_foundation::{base::TCFType, string::CFString};
use core_foundation_sys::{base::Boolean, string::CFStringRef};

type AXError = i32;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementIsAttributeSettable(
        element: AXUIElementRef,
        attribute: CFStringRef,
        settable: *mut Boolean,
    ) -> AXError;
}

pub(super) fn selected_text_writable(element: AXUIElementRef) -> Result<bool, AxFailure> {
    attribute_settable(
        element,
        "AXSelectedText",
        AxStage::SelectedTextSettable,
        ExtractionOrigin::SelectedText,
    )
}

pub(super) fn diagnose_selected_range_writable(element: AXUIElementRef) {
    let _ = attribute_settable(
        element,
        "AXSelectedTextRange",
        AxStage::SelectedRangeSettable,
        ExtractionOrigin::CfRange,
    );
}

fn attribute_settable(
    element: AXUIElementRef,
    name: &str,
    stage: AxStage,
    origin: ExtractionOrigin,
) -> Result<bool, AxFailure> {
    let attribute = CFString::new(name);
    let mut settable: Boolean = 0;
    let status = unsafe {
        AXUIElementIsAttributeSettable(element, attribute.as_concrete_TypeRef(), &mut settable)
    };
    let category = AxCategory::from_status(status);
    if status == AX_SUCCESS {
        let category = if settable != 0 {
            AxCategory::Settable
        } else {
            AxCategory::NotSettable
        };
        crate::diagnostics::ax_resolution(stage, origin, category);
        return Ok(settable != 0);
    }
    crate::diagnostics::ax_resolution(stage, origin, category);
    if matches!(
        category,
        AxCategory::NoValue | AxCategory::AttributeUnsupported
    ) {
        Ok(false)
    } else {
        Err(AxFailure::new(stage, category))
    }
}
