use super::{
    macos_ax::{self, AXUIElementRef, OwnedCfValue, AX_SUCCESS},
    macos_focus::{AxCategory, AxFailure, AxStage, ExtractionOrigin, RangeRepresentation},
};
use core_foundation::base::{CFGetTypeID, TCFType};
use std::ffi::c_void;

type AXValueRef = *const c_void;
type AXError = i32;

const AX_VALUE_CF_RANGE: i32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CFRange {
    pub(super) location: isize,
    pub(super) length: isize,
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXValueCreate(value_type: i32, value: *const c_void) -> AXValueRef;
    fn AXValueGetType(value: AXValueRef) -> i32;
    fn AXValueGetTypeID() -> usize;
    fn AXValueGetValue(value: AXValueRef, value_type: i32, output: *mut c_void) -> u8;
}

pub(super) fn selected_range(element: AXUIElementRef) -> Result<CFRange, AxFailure> {
    let origin = ExtractionOrigin::CfRange;
    let stage = AxStage::SelectedRange;
    let value = macos_ax::attribute(element, "AXSelectedTextRange", stage, origin)?;
    match representation(&value, origin) {
        RangeRepresentation::CfRange => {}
        RangeRepresentation::TextMarker => {
            return Err(AxFailure::new(stage, AxCategory::TextMarkerRange));
        }
        RangeRepresentation::Unsupported => {
            return Err(AxFailure::new(stage, AxCategory::UnexpectedType));
        }
    }
    let mut range = CFRange::default();
    let success = unsafe {
        AXValueGetValue(
            value.as_ref(),
            AX_VALUE_CF_RANGE,
            (&mut range as *mut CFRange).cast(),
        )
    };
    if success == 0 {
        return Err(AxFailure::new(stage, AxCategory::UnexpectedType));
    }
    if range.location < 0 || range.length <= 0 {
        return Err(AxFailure::new(stage, AxCategory::EmptyRange));
    }
    Ok(range)
}

pub(super) fn string_for_range(
    element: AXUIElementRef,
    range: CFRange,
) -> Result<String, AxFailure> {
    let stage = AxStage::StringForRange;
    let origin = ExtractionOrigin::CfRange;
    let parameter = range_value(range)?;
    let value = macos_ax::parameterized_attribute(
        element,
        "AXStringForRange",
        parameter.as_ref(),
        stage,
        origin,
    )?;
    macos_ax::string_value(&value, stage, origin)
}

fn range_value(range: CFRange) -> Result<OwnedCfValue, AxFailure> {
    let value = unsafe { AXValueCreate(AX_VALUE_CF_RANGE, (&range as *const CFRange).cast()) };
    OwnedCfValue::from_created(value, AxStage::StringForRange)
}

fn representation(value: &OwnedCfValue, origin: ExtractionOrigin) -> RangeRepresentation {
    let type_id = unsafe { CFGetTypeID(value.as_ref()) };
    if type_id == unsafe { AXValueGetTypeID() } {
        let value_type = unsafe { AXValueGetType(value.as_ref()) };
        let category = AxCategory::from_value_type(value_type);
        crate::diagnostics::ax_resolution(AxStage::SelectedRangeType, origin, category);
        if value_type == AX_VALUE_CF_RANGE {
            RangeRepresentation::CfRange
        } else {
            RangeRepresentation::Unsupported
        }
    } else if type_id == text_marker_range_type_id() {
        RangeRepresentation::TextMarker
    } else {
        RangeRepresentation::Unsupported
    }
}

fn text_marker_range_type_id() -> usize {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXTextMarkerRangeGetTypeID() -> usize;
    }
    unsafe { AXTextMarkerRangeGetTypeID() }
}

pub(super) fn marker_eligible_after_range(failure: AxFailure) -> bool {
    match failure.stage {
        AxStage::SelectedRange => matches!(
            failure.category,
            AxCategory::NoValue
                | AxCategory::AttributeUnsupported
                | AxCategory::EmptyRange
                | AxCategory::TextMarkerRange
        ),
        AxStage::StringForRange => matches!(
            failure.category,
            AxCategory::NoValue
                | AxCategory::AttributeUnsupported
                | AxCategory::ParameterizedAttributeUnsupported
        ),
        _ => false,
    }
}

pub(super) fn set_selected_range(element: AXUIElementRef, range: CFRange) -> Result<(), AxFailure> {
    let value = range_value(range)?;
    let attribute = core_foundation::string::CFString::new("AXSelectedTextRange");
    let status = unsafe {
        AXUIElementSetAttributeValue(element, attribute.as_concrete_TypeRef(), value.as_ref())
    };
    (status == AX_SUCCESS)
        .then_some(())
        .ok_or_else(|| AxFailure::new(AxStage::SelectedRange, AxCategory::from_status(status)))
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: core_foundation::string::CFStringRef,
        value: core_foundation::base::CFTypeRef,
    ) -> AXError;
}
