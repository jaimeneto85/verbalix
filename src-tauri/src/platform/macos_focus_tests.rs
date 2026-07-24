use super::macos_focus::*;

#[test]
fn maps_known_ax_statuses_without_exposing_raw_values() {
    let known = [
        (0, AxCategory::Success),
        (-25200, AxCategory::Failure),
        (-25201, AxCategory::IllegalArgument),
        (-25202, AxCategory::InvalidUiElement),
        (-25203, AxCategory::InvalidObserver),
        (-25204, AxCategory::CannotComplete),
        (-25205, AxCategory::AttributeUnsupported),
        (-25206, AxCategory::ActionUnsupported),
        (-25207, AxCategory::NotificationUnsupported),
        (-25208, AxCategory::NotImplemented),
        (-25209, AxCategory::NotificationAlreadyRegistered),
        (-25210, AxCategory::NotificationNotRegistered),
        (-25211, AxCategory::ApiDisabled),
        (-25212, AxCategory::NoValue),
        (-25213, AxCategory::ParameterizedAttributeUnsupported),
        (-25214, AxCategory::NotEnoughPrecision),
    ];

    for (status, category) in known {
        assert_eq!(AxCategory::from_status(status), category);
    }
    assert_eq!(AxCategory::from_status(-7), AxCategory::Unknown);
}

#[test]
fn diagnostic_labels_are_stable_and_sanitized() {
    let stages = [
        (AxStage::Trust, "trust"),
        (
            AxStage::SystemWideFocusedElement,
            "system_wide_focused_element",
        ),
        (AxStage::Role, "role"),
        (AxStage::SelectedText, "selected_text"),
        (AxStage::SelectedTextSettable, "selected_text_settable"),
        (AxStage::SelectedRangeSettable, "selected_range_settable"),
        (AxStage::Identifier, "identifier"),
        (AxStage::SelectedRange, "selected_range"),
        (AxStage::SelectedRangeType, "selected_range_type"),
        (AxStage::StringForRange, "string_for_range"),
        (AxStage::Value, "value"),
        (AxStage::ValueType, "value_type"),
        (AxStage::ValueLength, "value_length"),
        (AxStage::RangeStability, "range_stability"),
        (
            AxStage::SelectedTextMarkerRange,
            "selected_text_marker_range",
        ),
        (
            AxStage::StringForTextMarkerRange,
            "string_for_text_marker_range",
        ),
        (
            AxStage::BoundsForTextMarkerRange,
            "bounds_for_text_marker_range",
        ),
        (AxStage::IndexForTextMarker, "index_for_text_marker"),
        (
            AxStage::LengthForTextMarkerRange,
            "length_for_text_marker_range",
        ),
        (AxStage::Pid, "pid"),
        (AxStage::Geometry, "geometry"),
    ];
    let origins = [
        (ExtractionOrigin::SelectedText, "selected_text"),
        (ExtractionOrigin::CfRange, "cf_range"),
        (ExtractionOrigin::ValueRange, "value_range"),
        (ExtractionOrigin::TextMarker, "text_marker"),
    ];

    for (stage, label) in stages {
        assert_eq!(stage.as_str(), label);
    }
    for (origin, label) in origins {
        assert_eq!(origin.as_str(), label);
    }
    assert_eq!(AxCategory::CannotComplete.as_str(), "cannot_complete");
}

#[test]
fn marker_fallback_is_closed_to_structural_failures() {
    let categories = [
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
    ];

    for category in categories {
        assert_eq!(
            marker_fallback(AxFailure::new(AxStage::SelectedText, category)),
            matches!(
                category,
                AxCategory::NoValue | AxCategory::AttributeUnsupported
            )
        );
    }
    assert!(!marker_fallback(AxFailure::new(
        AxStage::SelectedRange,
        AxCategory::NoValue,
    )));
}

#[test]
fn ax_value_types_only_authorize_classic_cf_ranges() {
    let expected = [
        (1, AxCategory::Point),
        (2, AxCategory::Size),
        (3, AxCategory::Rect),
        (4, AxCategory::CfRange),
        (5, AxCategory::AxErrorValue),
        (0, AxCategory::IllegalValueType),
        (99, AxCategory::IllegalValueType),
    ];

    for (value_type, category) in expected {
        assert_eq!(AxCategory::from_value_type(value_type), category);
    }
}
