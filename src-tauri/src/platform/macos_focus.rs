#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum AxStage {
    Trust,
    SystemWideFocusedElement,
    Role,
    SelectedText,
    SelectedTextSettable,
    SelectedTextWrite,
    SelectedRangeSettable,
    Identifier,
    SelectedRange,
    SelectedRangeType,
    StringForRange,
    Value,
    ValueType,
    ValueLength,
    RangeStability,
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
            Self::SelectedTextWrite => "selected_text_write",
            Self::SelectedRangeSettable => "selected_range_settable",
            Self::Identifier => "identifier",
            Self::SelectedRange => "selected_range",
            Self::SelectedRangeType => "selected_range_type",
            Self::StringForRange => "string_for_range",
            Self::Value => "value",
            Self::ValueType => "value_type",
            Self::ValueLength => "value_length",
            Self::RangeStability => "range_stability",
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
    TextMarkerRange,
    Point,
    Size,
    Rect,
    AxErrorValue,
    IllegalValueType,
    EmptyRange,
    InvalidRange,
    RangeChanged,
    LimitExceeded,
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
            Self::TextMarkerRange => "text_marker_range",
            Self::Point => "point",
            Self::Size => "size",
            Self::Rect => "rect",
            Self::AxErrorValue => "ax_error",
            Self::IllegalValueType => "illegal_value_type",
            Self::EmptyRange => "empty_range",
            Self::InvalidRange => "invalid_range",
            Self::RangeChanged => "range_changed",
            Self::LimitExceeded => "limit_exceeded",
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
    ValueRange,
    TextMarker,
}

impl ExtractionOrigin {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SelectedText => "selected_text",
            Self::CfRange => "cf_range",
            Self::ValueRange => "value_range",
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

pub(crate) fn marker_fallback(failure: AxFailure) -> bool {
    failure.stage == AxStage::SelectedText
        && matches!(
            failure.category,
            AxCategory::NoValue | AxCategory::AttributeUnsupported
        )
}
