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
    match classify_settable(status, settable) {
        Ok((is_settable, category)) => {
            crate::diagnostics::ax_resolution(stage, origin, category);
            Ok(is_settable)
        }
        Err(category) => {
            crate::diagnostics::ax_resolution(stage, origin, category);
            Err(AxFailure::new(stage, category))
        }
    }
}

fn classify_settable(status: AXError, settable: Boolean) -> Result<(bool, AxCategory), AxCategory> {
    let category = AxCategory::from_status(status);
    if status == AX_SUCCESS {
        return Ok(if settable != 0 {
            (true, AxCategory::Settable)
        } else {
            (false, AxCategory::NotSettable)
        });
    }
    if matches!(
        category,
        AxCategory::NoValue | AxCategory::AttributeUnsupported
    ) {
        Ok((false, category))
    } else {
        Err(category)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_and_write_capabilities_remain_independent() {
        assert_eq!(
            classify_settable(AX_SUCCESS, 1),
            Ok((true, AxCategory::Settable))
        );
        assert_eq!(
            classify_settable(AX_SUCCESS, 0),
            Ok((false, AxCategory::NotSettable))
        );
        for status in [-25205, -25212] {
            assert!(matches!(classify_settable(status, 1), Ok((false, _))));
        }
        for status in [-25204, -25211, -25202, -1] {
            assert!(classify_settable(status, 0).is_err());
        }
    }
}
