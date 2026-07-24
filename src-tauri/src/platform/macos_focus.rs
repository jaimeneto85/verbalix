#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum AxStage {
    Trust,
    SystemWideFocusedElement,
    Role,
    SelectedText,
    SelectedTextSettable,
    SelectedRange,
    SelectedRangeType,
    StringForRange,
    SelectedTextMarkerRange,
    StringForTextMarkerRange,
    BoundsForTextMarkerRange,
    IndexForTextMarker,
    LengthForTextMarkerRange,
    Pid,
    Geometry,
}

impl AxStage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Trust => "trust",
            Self::SystemWideFocusedElement => "system_wide_focused_element",
            Self::Role => "role",
            Self::SelectedText => "selected_text",
            Self::SelectedTextSettable => "selected_text_settable",
            Self::SelectedRange => "selected_range",
            Self::SelectedRangeType => "selected_range_type",
            Self::StringForRange => "string_for_range",
            Self::SelectedTextMarkerRange => "selected_text_marker_range",
            Self::StringForTextMarkerRange => "string_for_text_marker_range",
            Self::BoundsForTextMarkerRange => "bounds_for_text_marker_range",
            Self::IndexForTextMarker => "index_for_text_marker",
            Self::LengthForTextMarkerRange => "length_for_text_marker_range",
            Self::Pid => "pid",
            Self::Geometry => "geometry",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum AxCategory {
    Success,
    Failure,
    IllegalArgument,
    InvalidUiElement,
    InvalidObserver,
    CannotComplete,
    AttributeUnsupported,
    ActionUnsupported,
    NotificationUnsupported,
    NotImplemented,
    NotificationAlreadyRegistered,
    NotificationNotRegistered,
    ApiDisabled,
    NoValue,
    ParameterizedAttributeUnsupported,
    NotEnoughPrecision,
    NullValue,
    UnexpectedType,
    InvalidPid,
    SelfProcess,
    CfRange,
    Point,
    Size,
    Rect,
    AxErrorValue,
    IllegalValueType,
    EmptyRange,
    Settable,
    NotSettable,
    Unknown,
}

impl AxCategory {
    pub(crate) fn from_status(status: i32) -> Self {
        match status {
            0 => Self::Success,
            -25200 => Self::Failure,
            -25201 => Self::IllegalArgument,
            -25202 => Self::InvalidUiElement,
            -25203 => Self::InvalidObserver,
            -25204 => Self::CannotComplete,
            -25205 => Self::AttributeUnsupported,
            -25206 => Self::ActionUnsupported,
            -25207 => Self::NotificationUnsupported,
            -25208 => Self::NotImplemented,
            -25209 => Self::NotificationAlreadyRegistered,
            -25210 => Self::NotificationNotRegistered,
            -25211 => Self::ApiDisabled,
            -25212 => Self::NoValue,
            -25213 => Self::ParameterizedAttributeUnsupported,
            -25214 => Self::NotEnoughPrecision,
            _ => Self::Unknown,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::IllegalArgument => "illegal_argument",
            Self::InvalidUiElement => "invalid_ui_element",
            Self::InvalidObserver => "invalid_observer",
            Self::CannotComplete => "cannot_complete",
            Self::AttributeUnsupported => "attribute_unsupported",
            Self::ActionUnsupported => "action_unsupported",
            Self::NotificationUnsupported => "notification_unsupported",
            Self::NotImplemented => "not_implemented",
            Self::NotificationAlreadyRegistered => "notification_already_registered",
            Self::NotificationNotRegistered => "notification_not_registered",
            Self::ApiDisabled => "api_disabled",
            Self::NoValue => "no_value",
            Self::ParameterizedAttributeUnsupported => "parameterized_attribute_unsupported",
            Self::NotEnoughPrecision => "not_enough_precision",
            Self::NullValue => "null_value",
            Self::UnexpectedType => "unexpected_type",
            Self::InvalidPid => "invalid_pid",
            Self::SelfProcess => "self_process",
            Self::CfRange => "cf_range",
            Self::Point => "point",
            Self::Size => "size",
            Self::Rect => "rect",
            Self::AxErrorValue => "ax_error",
            Self::IllegalValueType => "illegal_value_type",
            Self::EmptyRange => "empty_range",
            Self::Settable => "settable",
            Self::NotSettable => "not_settable",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn from_value_type(value_type: i32) -> Self {
        match value_type {
            1 => Self::Point,
            2 => Self::Size,
            3 => Self::Rect,
            4 => Self::CfRange,
            5 => Self::AxErrorValue,
            _ => Self::IllegalValueType,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ExtractionOrigin {
    SelectedText,
    CfRange,
    TextMarker,
}

impl ExtractionOrigin {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SelectedText => "selected_text",
            Self::CfRange => "cf_range",
            Self::TextMarker => "text_marker",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RangeRepresentation {
    CfRange,
    TextMarker,
    Unsupported,
}

pub(crate) fn marker_fallback(category: AxCategory) -> bool {
    matches!(
        category,
        AxCategory::NoValue | AxCategory::AttributeUnsupported
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AxFailure {
    pub(crate) stage: AxStage,
    pub(crate) category: AxCategory,
}

impl AxFailure {
    pub(crate) fn new(stage: AxStage, category: AxCategory) -> Self {
        Self { stage, category }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            (AxStage::SelectedRange, "selected_range"),
            (AxStage::SelectedRangeType, "selected_range_type"),
            (AxStage::StringForRange, "string_for_range"),
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
            AxCategory::Point,
            AxCategory::Size,
            AxCategory::Rect,
            AxCategory::AxErrorValue,
            AxCategory::IllegalValueType,
            AxCategory::EmptyRange,
            AxCategory::Settable,
            AxCategory::NotSettable,
            AxCategory::Unknown,
        ];

        for category in categories {
            assert_eq!(
                marker_fallback(category),
                matches!(
                    category,
                    AxCategory::NoValue | AxCategory::AttributeUnsupported
                )
            );
        }
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
}
