use super::{
    macos_ax::{self, AXUIElementRef, OwnedCfValue},
    macos_classic_range::{self, CFRange},
    macos_focus::{AxCategory, AxFailure, AxStage, ExtractionOrigin},
};
use core_foundation::base::CFGetTypeID;
use core_foundation_sys::{
    base::CFRange as SystemCFRange,
    string::{
        CFStringGetCharacterAtIndex, CFStringGetCharacters, CFStringGetLength, CFStringGetTypeID,
        CFStringRef,
    },
};

const MAX_VALUE_UTF16_UNITS: usize = 262_144;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ValueRangeSelection {
    pub(super) text: String,
    pub(super) range: CFRange,
}

pub(super) fn extract(element: AXUIElementRef) -> Result<ValueRangeSelection, AxFailure> {
    let first =
        macos_classic_range::selected_range_with_origin(element, ExtractionOrigin::ValueRange)?;
    let value = macos_ax::attribute(
        element,
        "AXValue",
        AxStage::Value,
        ExtractionOrigin::ValueRange,
    )?;
    let value_length = validate_value(&value, first)?;
    let second =
        macos_classic_range::selected_range_with_origin(element, ExtractionOrigin::ValueRange)?;
    if first != second {
        let failure = AxFailure::new(AxStage::RangeStability, AxCategory::RangeChanged);
        crate::diagnostics::ax_resolution(
            failure.stage,
            ExtractionOrigin::ValueRange,
            failure.category,
        );
        return Err(failure);
    }
    crate::diagnostics::ax_resolution(
        AxStage::RangeStability,
        ExtractionOrigin::ValueRange,
        AxCategory::Success,
    );
    let text = copy_selected_range(&value, first, value_length)?;
    Ok(ValueRangeSelection { text, range: first })
}

pub(super) fn fallback_eligible(failure: AxFailure) -> bool {
    failure.stage == AxStage::StringForRange
        && matches!(
            failure.category,
            AxCategory::NoValue
                | AxCategory::AttributeUnsupported
                | AxCategory::ParameterizedAttributeUnsupported
        )
}

pub(super) fn marker_eligible(failure: AxFailure) -> bool {
    failure.stage == AxStage::Value
        && matches!(
            failure.category,
            AxCategory::NoValue | AxCategory::AttributeUnsupported
        )
}

pub(super) fn role_eligible(role: &str) -> bool {
    matches!(role, "AXTextArea" | "AXTextField" | "AXStaticText")
}

fn validate_value(value: &OwnedCfValue, range: CFRange) -> Result<usize, AxFailure> {
    if unsafe { CFGetTypeID(value.as_ref()) != CFStringGetTypeID() } {
        return diagnostic_failure(AxStage::ValueType, AxCategory::UnexpectedType);
    }
    crate::diagnostics::ax_resolution(
        AxStage::ValueType,
        ExtractionOrigin::ValueRange,
        AxCategory::Success,
    );
    let length = unsafe { CFStringGetLength(as_string(value)) };
    let length = usize::try_from(length)
        .map_err(|_| failure(AxStage::ValueLength, AxCategory::InvalidRange))?;
    if length > MAX_VALUE_UTF16_UNITS {
        return diagnostic_failure(AxStage::ValueLength, AxCategory::LimitExceeded);
    }
    crate::diagnostics::ax_resolution(
        AxStage::ValueLength,
        ExtractionOrigin::ValueRange,
        AxCategory::Success,
    );
    validate_range(range, length)?;
    validate_scalar_boundaries(value, range, length)?;
    Ok(length)
}

fn copy_selected_range(
    value: &OwnedCfValue,
    range: CFRange,
    value_length: usize,
) -> Result<String, AxFailure> {
    let (_, selection_length, _) = validated_offsets(range, value_length)?;
    let mut units = vec![0_u16; selection_length];
    unsafe { CFStringGetCharacters(as_string(value), system_range(range)?, units.as_mut_ptr()) };
    String::from_utf16(&units).map_err(|_| failure(AxStage::Value, AxCategory::InvalidRange))
}

fn validate_range(range: CFRange, value_length: usize) -> Result<(), AxFailure> {
    validated_offsets(range, value_length).map(|_| ())
}

fn validated_offsets(
    range: CFRange,
    value_length: usize,
) -> Result<(usize, usize, usize), AxFailure> {
    let start = usize::try_from(range.location)
        .map_err(|_| failure(AxStage::ValueLength, AxCategory::InvalidRange))?;
    let length = usize::try_from(range.length)
        .map_err(|_| failure(AxStage::ValueLength, AxCategory::InvalidRange))?;
    let end = start
        .checked_add(length)
        .ok_or_else(|| failure(AxStage::ValueLength, AxCategory::InvalidRange))?;
    if length == 0 || end > value_length {
        return Err(failure(AxStage::ValueLength, AxCategory::InvalidRange));
    }
    Ok((start, length, end))
}

fn validate_scalar_boundaries(
    value: &OwnedCfValue,
    range: CFRange,
    value_length: usize,
) -> Result<(), AxFailure> {
    let (start, _, end) = validated_offsets(range, value_length)?;
    if (start < value_length && is_low_surrogate(character(value, start)?))
        || (end < value_length && is_low_surrogate(character(value, end)?))
    {
        return Err(failure(AxStage::Value, AxCategory::InvalidRange));
    }
    Ok(())
}

fn character(value: &OwnedCfValue, index: usize) -> Result<u16, AxFailure> {
    let index =
        isize::try_from(index).map_err(|_| failure(AxStage::Value, AxCategory::InvalidRange))?;
    Ok(unsafe { CFStringGetCharacterAtIndex(as_string(value), index) })
}

fn is_low_surrogate(unit: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&unit)
}

fn system_range(range: CFRange) -> Result<SystemCFRange, AxFailure> {
    Ok(SystemCFRange {
        location: range.location,
        length: range.length,
    })
}

fn as_string(value: &OwnedCfValue) -> CFStringRef {
    value.as_ref().cast()
}

fn failure(stage: AxStage, category: AxCategory) -> AxFailure {
    crate::diagnostics::ax_resolution(stage, ExtractionOrigin::ValueRange, category);
    AxFailure::new(stage, category)
}

fn diagnostic_failure<T>(stage: AxStage, category: AxCategory) -> Result<T, AxFailure> {
    Err(failure(stage, category))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_checked_utf16_offsets_without_byte_indexing() {
        assert_eq!(
            validated_offsets(
                CFRange {
                    location: 1,
                    length: 4
                },
                6
            ),
            Ok((1, 4, 5))
        );
        for range in [
            CFRange {
                location: -1,
                length: 1,
            },
            CFRange {
                location: 0,
                length: 0,
            },
            CFRange {
                location: 4,
                length: 3,
            },
            CFRange {
                location: isize::MAX,
                length: isize::MAX,
            },
        ] {
            assert!(validated_offsets(range, 6).is_err());
        }
    }

    #[test]
    fn fallback_categories_are_explicit_and_stage_bound() {
        assert!(fallback_eligible(AxFailure::new(
            AxStage::StringForRange,
            AxCategory::ParameterizedAttributeUnsupported
        )));
        assert!(!fallback_eligible(AxFailure::new(
            AxStage::StringForRange,
            AxCategory::CannotComplete
        )));
        assert!(marker_eligible(AxFailure::new(
            AxStage::Value,
            AxCategory::AttributeUnsupported
        )));
        assert!(!marker_eligible(AxFailure::new(
            AxStage::ValueType,
            AxCategory::UnexpectedType
        )));
    }

    #[test]
    fn value_range_roles_are_conservative() {
        for role in ["AXTextArea", "AXTextField", "AXStaticText"] {
            assert!(role_eligible(role));
        }
        for role in ["AXSecureTextField", "AXWebArea", "AXButton", ""] {
            assert!(!role_eligible(role));
        }
    }
}
