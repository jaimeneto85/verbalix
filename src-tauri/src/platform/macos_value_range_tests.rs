use super::*;
use core_foundation::{
    base::{CFRetain, TCFType},
    number::CFNumber,
    string::CFString,
};

fn range(location: isize, length: isize) -> CFRange {
    CFRange { location, length }
}

fn selected(value: &str, selection: CFRange) -> Result<String, AxFailure> {
    let value = CFString::new(value);
    let value = value.as_concrete_TypeRef();
    let length = validate_string(value, selection)?;
    copy_string_range(value, selection, length)
}

fn categories() -> &'static [AxCategory] {
    &[
        AxCategory::Success,
        AxCategory::Failure,
        AxCategory::IllegalArgument,
        AxCategory::InvalidUiElement,
        AxCategory::InvalidObserver,
        AxCategory::CannotComplete,
        AxCategory::AttributeUnsupported,
        AxCategory::ActionUnsupported,
        AxCategory::NotificationUnsupported,
        AxCategory::NotImplemented,
        AxCategory::NotificationAlreadyRegistered,
        AxCategory::NotificationNotRegistered,
        AxCategory::ApiDisabled,
        AxCategory::NoValue,
        AxCategory::ParameterizedAttributeUnsupported,
        AxCategory::NotEnoughPrecision,
        AxCategory::NullValue,
        AxCategory::UnexpectedType,
        AxCategory::InvalidPid,
        AxCategory::SelfProcess,
        AxCategory::CfRange,
        AxCategory::TextMarkerRange,
        AxCategory::Point,
        AxCategory::Size,
        AxCategory::Rect,
        AxCategory::AxErrorValue,
        AxCategory::IllegalValueType,
        AxCategory::EmptyRange,
        AxCategory::InvalidRange,
        AxCategory::RangeChanged,
        AxCategory::LimitExceeded,
        AxCategory::Settable,
        AxCategory::NotSettable,
        AxCategory::Unknown,
    ]
}

#[test]
fn copies_bmp_emoji_and_combining_ranges_by_utf16_units() {
    let value = "A👩🏽‍💻e\u{301}Z";

    assert_eq!(selected(value, range(0, 1)).unwrap(), "A");
    assert_eq!(selected(value, range(1, 7)).unwrap(), "👩🏽‍💻");
    assert_eq!(selected(value, range(8, 2)).unwrap(), "e\u{301}");
    assert_eq!(selected(value, range(0, 11)).unwrap(), value);
}

#[test]
fn rejects_empty_negative_overflow_out_of_bounds_and_surrogate_splits() {
    let value = "A👩Z";

    for selection in [
        range(-1, 1),
        range(0, 0),
        range(4, 1),
        range(isize::MAX, isize::MAX),
        range(2, 1),
        range(1, 1),
    ] {
        assert!(matches!(
            selected(value, selection),
            Err(AxFailure {
                category: AxCategory::InvalidRange,
                ..
            })
        ));
    }
}

#[test]
fn value_limit_accepts_the_boundary_and_rejects_one_unit_more() {
    let exact = CFString::new(&"a".repeat(MAX_VALUE_UTF16_UNITS));
    let exact_ref = exact.as_concrete_TypeRef();
    assert_eq!(
        validate_string(exact_ref, range(MAX_VALUE_UTF16_UNITS as isize - 1, 1)),
        Ok(MAX_VALUE_UTF16_UNITS)
    );

    let oversized = CFString::new(&"a".repeat(MAX_VALUE_UTF16_UNITS + 1));
    assert_eq!(
        validate_string(oversized.as_concrete_TypeRef(), range(0, 1)),
        Err(AxFailure::new(
            AxStage::ValueLength,
            AxCategory::LimitExceeded
        ))
    );
}

#[test]
fn non_cfstring_value_is_rejected_before_length_or_copy() {
    let number = CFNumber::from(7);
    let retained = unsafe { CFRetain(number.as_CFTypeRef()) };
    let value = OwnedCfValue::from_created(retained, AxStage::Value).unwrap();

    assert_eq!(
        validate_value(&value, range(0, 1)),
        Err(AxFailure::new(
            AxStage::ValueType,
            AxCategory::UnexpectedType
        ))
    );
}

#[test]
fn changed_range_fails_before_selected_text_can_be_copied() {
    let current = range(4, 3);

    assert!(validate_stable_range(current, current).is_ok());
    assert_eq!(
        validate_stable_range(current, range(5, 3)),
        Err(AxFailure::new(
            AxStage::RangeStability,
            AxCategory::RangeChanged
        ))
    );
}

#[test]
fn value_fallback_accepts_only_string_for_range_capability_failures() {
    for &category in categories() {
        assert_eq!(
            fallback_eligible(AxFailure::new(AxStage::StringForRange, category)),
            matches!(
                category,
                AxCategory::NoValue
                    | AxCategory::AttributeUnsupported
                    | AxCategory::ParameterizedAttributeUnsupported
            )
        );
        assert!(!fallback_eligible(AxFailure::new(
            AxStage::SelectedRange,
            category
        )));
    }
}

#[test]
fn marker_fallback_accepts_only_value_capability_failures() {
    for &category in categories() {
        assert_eq!(
            marker_eligible(AxFailure::new(AxStage::Value, category)),
            matches!(
                category,
                AxCategory::NoValue | AxCategory::AttributeUnsupported
            )
        );
        assert!(!marker_eligible(AxFailure::new(
            AxStage::ValueType,
            category
        )));
    }
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
