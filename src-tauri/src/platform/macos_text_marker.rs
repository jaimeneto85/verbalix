use super::{
    macos_ax::{self, AXUIElementRef, OwnedCfValue},
    macos_classic_range::CFRange,
    macos_focus::{AxCategory, AxFailure, AxStage, ExtractionOrigin},
    macos_geometry,
};
use crate::domain::{Rect, VerbalixError};
use core_foundation::{
    base::{CFGetTypeID, TCFType},
    number::{CFNumber, CFNumberGetTypeID},
};
use std::ffi::c_void;

type AXTextMarkerRef = *const c_void;
type AXTextMarkerRangeRef = *const c_void;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXTextMarkerGetTypeID() -> usize;
    fn AXTextMarkerRangeGetTypeID() -> usize;
    fn AXTextMarkerRangeCopyStartMarker(range: AXTextMarkerRangeRef) -> AXTextMarkerRef;
    fn AXTextMarkerRangeCopyEndMarker(range: AXTextMarkerRangeRef) -> AXTextMarkerRef;
}

pub(super) struct TextMarkerSelection {
    pub(super) text: String,
    pub(super) range: CFRange,
    pub(super) bounds: Rect,
}

pub(super) fn extract(element: AXUIElementRef) -> Result<TextMarkerSelection, AxFailure> {
    let origin = ExtractionOrigin::TextMarker;
    let marker_range = macos_ax::attribute(
        element,
        "AXSelectedTextMarkerRange",
        AxStage::SelectedTextMarkerRange,
        origin,
    )?;
    if unsafe { CFGetTypeID(marker_range.as_ref()) != AXTextMarkerRangeGetTypeID() } {
        return Err(AxFailure::new(
            AxStage::SelectedTextMarkerRange,
            AxCategory::UnexpectedType,
        ));
    }
    let text = marker_string(element, &marker_range)?;
    let bounds = marker_bounds(element, &marker_range)?;
    let range = marker_range_indices(element, &marker_range)?;
    let utf16_length = isize::try_from(text.encode_utf16().count()).map_err(|_| {
        AxFailure::new(
            AxStage::LengthForTextMarkerRange,
            AxCategory::NotEnoughPrecision,
        )
    })?;
    if utf16_length != range.length {
        return Err(AxFailure::new(
            AxStage::LengthForTextMarkerRange,
            AxCategory::NotEnoughPrecision,
        ));
    }
    Ok(TextMarkerSelection {
        text,
        range,
        bounds,
    })
}

fn marker_string(
    element: AXUIElementRef,
    marker_range: &OwnedCfValue,
) -> Result<String, AxFailure> {
    let stage = AxStage::StringForTextMarkerRange;
    let origin = ExtractionOrigin::TextMarker;
    let value = macos_ax::parameterized_attribute(
        element,
        "AXStringForTextMarkerRange",
        marker_range.as_ref(),
        stage,
        origin,
    )?;
    macos_ax::string_value(&value, stage, origin)
}

fn marker_bounds(element: AXUIElementRef, marker_range: &OwnedCfValue) -> Result<Rect, AxFailure> {
    let stage = AxStage::BoundsForTextMarkerRange;
    let origin = ExtractionOrigin::TextMarker;
    let value = macos_ax::parameterized_attribute(
        element,
        "AXBoundsForTextMarkerRange",
        marker_range.as_ref(),
        stage,
        origin,
    )?;
    macos_geometry::rect_from_value(value.as_ref())
        .ok_or_else(|| AxFailure::new(stage, AxCategory::UnexpectedType))
}

fn marker_range_indices(
    element: AXUIElementRef,
    marker_range: &OwnedCfValue,
) -> Result<CFRange, AxFailure> {
    let start =
        unsafe { AXTextMarkerRangeCopyStartMarker(marker_range.as_ref() as AXTextMarkerRangeRef) };
    let start = OwnedCfValue::from_created(start, AxStage::IndexForTextMarker)?;
    let end =
        unsafe { AXTextMarkerRangeCopyEndMarker(marker_range.as_ref() as AXTextMarkerRangeRef) };
    let end = OwnedCfValue::from_created(end, AxStage::IndexForTextMarker)?;
    let marker_type = unsafe { AXTextMarkerGetTypeID() };
    if unsafe { CFGetTypeID(start.as_ref()) } != marker_type
        || unsafe { CFGetTypeID(end.as_ref()) } != marker_type
    {
        return Err(AxFailure::new(
            AxStage::IndexForTextMarker,
            AxCategory::UnexpectedType,
        ));
    }
    let start_index = marker_number(
        element,
        "AXIndexForTextMarker",
        &start,
        AxStage::IndexForTextMarker,
    )?;
    let end_index = marker_number(
        element,
        "AXIndexForTextMarker",
        &end,
        AxStage::IndexForTextMarker,
    )?;
    let reported_length = marker_number(
        element,
        "AXLengthForTextMarkerRange",
        marker_range,
        AxStage::LengthForTextMarkerRange,
    )?;
    marker_cf_range(start_index, end_index, reported_length).map_err(|_| {
        AxFailure::new(
            AxStage::LengthForTextMarkerRange,
            AxCategory::NotEnoughPrecision,
        )
    })
}

fn marker_number(
    element: AXUIElementRef,
    attribute: &str,
    parameter: &OwnedCfValue,
    stage: AxStage,
) -> Result<i64, AxFailure> {
    let origin = ExtractionOrigin::TextMarker;
    let value =
        macos_ax::parameterized_attribute(element, attribute, parameter.as_ref(), stage, origin)?;
    if unsafe { CFGetTypeID(value.as_ref()) != CFNumberGetTypeID() } {
        return Err(AxFailure::new(stage, AxCategory::UnexpectedType));
    }
    let number = unsafe { CFNumber::wrap_under_get_rule(value.as_ref().cast()) };
    number
        .to_i64()
        .filter(|number| *number >= 0)
        .ok_or_else(|| AxFailure::new(stage, AxCategory::NotEnoughPrecision))
}

pub(super) fn marker_cf_range(
    start_index: i64,
    end_index: i64,
    reported_length: i64,
) -> Result<CFRange, VerbalixError> {
    let calculated_length = end_index
        .checked_sub(start_index)
        .filter(|length| *length > 0 && *length == reported_length)
        .ok_or(VerbalixError::SelectionUnavailable)?;
    let location = isize::try_from(start_index).map_err(|_| VerbalixError::SelectionUnavailable)?;
    let length =
        isize::try_from(calculated_length).map_err(|_| VerbalixError::SelectionUnavailable)?;
    if location < 0 {
        return Err(VerbalixError::SelectionUnavailable);
    }
    Ok(CFRange { location, length })
}
