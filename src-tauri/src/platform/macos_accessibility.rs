use super::macos_focus::{
    marker_fallback, AxCategory, AxFailure, AxStage, ExtractionOrigin, RangeRepresentation,
};
use crate::{
    application::SelectionPort,
    domain::{GeometrySource, Rect, SelectionSnapshot, TextRange, VerbalixError},
};
use core_foundation::{
    base::{CFGetTypeID, CFRelease, CFTypeRef, TCFType},
    boolean::CFBoolean,
    dictionary::CFDictionary,
    number::{CFNumber, CFNumberGetTypeID},
    string::{CFString, CFStringGetTypeID, CFStringRef},
};
use core_foundation_sys::{base::Boolean, dictionary::CFDictionaryRef};
use std::{
    ffi::c_void,
    mem,
    ptr::{self, NonNull},
    sync::Arc,
};

pub(super) type AXUIElementRef = *const c_void;
type AXValueRef = *const c_void;
type AXTextMarkerRef = *const c_void;
type AXTextMarkerRangeRef = *const c_void;
type AXError = i32;

pub(super) const AX_SUCCESS: AXError = 0;
const AX_VALUE_CF_RANGE: i32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CFRange {
    location: isize,
    length: isize,
}

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
    fn AXUIElementIsAttributeSettable(
        element: AXUIElementRef,
        attribute: CFStringRef,
        settable: *mut Boolean,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;
    fn AXValueGetType(value: AXValueRef) -> i32;
    fn AXValueGetTypeID() -> usize;
    fn AXValueGetValue(value: AXValueRef, value_type: i32, output: *mut c_void) -> Boolean;
    fn AXTextMarkerGetTypeID() -> usize;
    fn AXTextMarkerRangeGetTypeID() -> usize;
    fn AXTextMarkerRangeCopyStartMarker(range: AXTextMarkerRangeRef) -> AXTextMarkerRef;
    fn AXTextMarkerRangeCopyEndMarker(range: AXTextMarkerRangeRef) -> AXTextMarkerRef;
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

struct OwnedCfValue(NonNull<c_void>);

impl OwnedCfValue {
    fn from_created(value: CFTypeRef, stage: AxStage) -> Result<Self, AxFailure> {
        NonNull::new(value.cast_mut())
            .map(Self)
            .ok_or_else(|| AxFailure::new(stage, AxCategory::NullValue))
    }

    fn as_ref(&self) -> CFTypeRef {
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

struct ExtractedSelection {
    text: String,
    range: CFRange,
    bounds: Rect,
    geometry_source: GeometrySource,
    writable: bool,
}

pub struct MacAccessibility;

impl MacAccessibility {
    pub fn new() -> Self {
        Self
    }

    pub fn start_observer(&self, callback: Arc<dyn Fn() + Send + Sync>) {
        super::macos_observer::start(callback);
    }

    fn attribute(
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
        Self::owned_copy_result(value, status, stage, origin)
    }

    fn parameterized_attribute(
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
        Self::owned_copy_result(value, status, stage, origin)
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
        let value = OwnedCfValue::from_created(value, stage).map_err(|failure| {
            crate::diagnostics::ax_resolution(stage, origin, failure.category);
            failure
        })?;
        crate::diagnostics::ax_resolution(stage, origin, AxCategory::Success);
        Ok(value)
    }

    pub(super) fn focused_element() -> Result<OwnedAxElement, VerbalixError> {
        let origin = ExtractionOrigin::SelectedText;
        let system = unsafe { AXUIElementCreateSystemWide() };
        let system = OwnedAxElement::from_created(system, AxStage::SystemWideFocusedElement)
            .map_err(|_| VerbalixError::SelectionUnavailable)?;
        Self::attribute(
            system.as_ref(),
            "AXFocusedUIElement",
            AxStage::SystemWideFocusedElement,
            origin,
        )
        .and_then(|value| value.into_ax_element(AxStage::SystemWideFocusedElement, origin))
        .map_err(|_| VerbalixError::SelectionUnavailable)
    }

    pub(super) fn string_attribute(
        element: AXUIElementRef,
        name: &str,
    ) -> Result<String, VerbalixError> {
        Self::string_attribute_at(
            element,
            name,
            AxStage::SelectedText,
            ExtractionOrigin::SelectedText,
        )
        .map_err(|_| VerbalixError::SelectionUnavailable)
    }

    fn string_attribute_at(
        element: AXUIElementRef,
        name: &str,
        stage: AxStage,
        origin: ExtractionOrigin,
    ) -> Result<String, AxFailure> {
        let value = Self::attribute(element, name, stage, origin)?;
        Self::string_value(&value, stage, origin)
    }

    fn string_value(
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

    fn selected_range(element: AXUIElementRef) -> Result<CFRange, AxFailure> {
        let origin = ExtractionOrigin::CfRange;
        let stage = AxStage::SelectedRange;
        let value = Self::attribute(element, "AXSelectedTextRange", stage, origin)?;
        if Self::range_representation(&value, origin) != RangeRepresentation::CfRange {
            return Err(AxFailure::new(stage, AxCategory::UnexpectedType));
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
            crate::diagnostics::ax_resolution(stage, origin, AxCategory::UnexpectedType);
            return Err(AxFailure::new(stage, AxCategory::UnexpectedType));
        }
        if range.location < 0 || range.length <= 0 {
            crate::diagnostics::ax_resolution(stage, origin, AxCategory::EmptyRange);
            return Err(AxFailure::new(stage, AxCategory::EmptyRange));
        }
        Ok(range)
    }

    fn range_representation(value: &OwnedCfValue, origin: ExtractionOrigin) -> RangeRepresentation {
        let type_id = unsafe { CFGetTypeID(value.as_ref()) };
        let representation = if type_id == unsafe { AXValueGetTypeID() } {
            let value_type = unsafe { AXValueGetType(value.as_ref()) };
            let category = AxCategory::from_value_type(value_type);
            crate::diagnostics::ax_resolution(AxStage::SelectedRangeType, origin, category);
            (value_type == AX_VALUE_CF_RANGE)
                .then_some(RangeRepresentation::CfRange)
                .unwrap_or(RangeRepresentation::Unsupported)
        } else if type_id == unsafe { AXTextMarkerRangeGetTypeID() } {
            crate::diagnostics::ax_resolution(
                AxStage::SelectedRangeType,
                origin,
                AxCategory::Success,
            );
            RangeRepresentation::TextMarker
        } else {
            crate::diagnostics::ax_resolution(
                AxStage::SelectedRangeType,
                origin,
                AxCategory::UnexpectedType,
            );
            RangeRepresentation::Unsupported
        };
        representation
    }

    fn direct_selection(
        element: AXUIElementRef,
        text: String,
    ) -> Result<ExtractedSelection, VerbalixError> {
        let range =
            Self::selected_range(element).map_err(|_| VerbalixError::SelectionUnavailable)?;
        let (bounds, geometry_source) =
            super::macos_geometry::resolve(element, range.location, range.length)
                .ok_or(VerbalixError::SelectionUnavailable)?;
        crate::diagnostics::ax_resolution(
            AxStage::Geometry,
            ExtractionOrigin::CfRange,
            AxCategory::Success,
        );
        Ok(ExtractedSelection {
            text,
            range,
            bounds,
            geometry_source,
            writable: Self::writable_at(element, ExtractionOrigin::SelectedText),
        })
    }

    fn cf_range_selection(element: AXUIElementRef) -> Result<ExtractedSelection, VerbalixError> {
        let origin = ExtractionOrigin::CfRange;
        let range =
            Self::selected_range(element).map_err(|_| VerbalixError::SelectionUnavailable)?;
        let value = Self::range_value(range)?;
        let text = Self::parameterized_attribute(
            element,
            "AXStringForRange",
            value.as_ref(),
            AxStage::StringForRange,
            origin,
        )
        .and_then(|value| Self::string_value(&value, AxStage::StringForRange, origin))
        .map_err(|_| VerbalixError::SelectionUnavailable)?;
        let (bounds, geometry_source) =
            super::macos_geometry::resolve(element, range.location, range.length)
                .ok_or(VerbalixError::SelectionUnavailable)?;
        crate::diagnostics::ax_resolution(AxStage::Geometry, origin, AxCategory::Success);
        Ok(ExtractedSelection {
            text,
            range,
            bounds,
            geometry_source,
            writable: false,
        })
    }

    fn range_value(range: CFRange) -> Result<OwnedCfValue, VerbalixError> {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXValueCreate(value_type: i32, value: *const c_void) -> AXValueRef;
        }
        let value = unsafe { AXValueCreate(AX_VALUE_CF_RANGE, (&range as *const CFRange).cast()) };
        OwnedCfValue::from_created(value, AxStage::StringForRange)
            .map_err(|_| VerbalixError::SelectionUnavailable)
    }

    fn marker_selection(element: AXUIElementRef) -> Result<ExtractedSelection, VerbalixError> {
        let origin = ExtractionOrigin::TextMarker;
        let marker_range = Self::attribute(
            element,
            "AXSelectedTextMarkerRange",
            AxStage::SelectedTextMarkerRange,
            origin,
        )
        .map_err(|_| VerbalixError::SelectionUnavailable)?;
        if Self::range_representation(&marker_range, origin) != RangeRepresentation::TextMarker {
            return Err(VerbalixError::SelectionUnavailable);
        }
        let text = Self::parameterized_attribute(
            element,
            "AXStringForTextMarkerRange",
            marker_range.as_ref(),
            AxStage::StringForTextMarkerRange,
            origin,
        )
        .and_then(|value| Self::string_value(&value, AxStage::StringForTextMarkerRange, origin))
        .map_err(|_| VerbalixError::SelectionUnavailable)?;
        let bounds_value = Self::parameterized_attribute(
            element,
            "AXBoundsForTextMarkerRange",
            marker_range.as_ref(),
            AxStage::BoundsForTextMarkerRange,
            origin,
        )
        .map_err(|_| VerbalixError::SelectionUnavailable)?;
        let bounds = super::macos_geometry::rect_from_value(bounds_value.as_ref())
            .ok_or(VerbalixError::SelectionUnavailable)?;
        crate::diagnostics::ax_resolution(AxStage::Geometry, origin, AxCategory::Success);
        let range = Self::marker_range(element, &marker_range)?;
        let utf16_length =
            isize::try_from(text.encode_utf16().count()).map_err(|_| VerbalixError::TextTooLong)?;
        if utf16_length != range.length {
            return Err(VerbalixError::SelectionUnavailable);
        }
        crate::diagnostics::ax_resolution(
            AxStage::SelectedTextSettable,
            origin,
            AxCategory::NotSettable,
        );
        Ok(ExtractedSelection {
            text,
            range,
            bounds,
            geometry_source: GeometrySource::TextMarkerRange,
            writable: false,
        })
    }

    fn marker_range(
        element: AXUIElementRef,
        marker_range: &OwnedCfValue,
    ) -> Result<CFRange, VerbalixError> {
        let origin = ExtractionOrigin::TextMarker;
        let start = unsafe {
            AXTextMarkerRangeCopyStartMarker(marker_range.as_ref() as AXTextMarkerRangeRef)
        };
        let start = OwnedCfValue::from_created(start, AxStage::IndexForTextMarker)
            .map_err(|_| VerbalixError::SelectionUnavailable)?;
        let end = unsafe {
            AXTextMarkerRangeCopyEndMarker(marker_range.as_ref() as AXTextMarkerRangeRef)
        };
        let end = OwnedCfValue::from_created(end, AxStage::IndexForTextMarker)
            .map_err(|_| VerbalixError::SelectionUnavailable)?;
        let marker_type = unsafe { AXTextMarkerGetTypeID() };
        if unsafe { CFGetTypeID(start.as_ref()) } != marker_type
            || unsafe { CFGetTypeID(end.as_ref()) } != marker_type
        {
            return Err(VerbalixError::SelectionUnavailable);
        }
        let start_index = Self::marker_number(
            element,
            "AXIndexForTextMarker",
            &start,
            AxStage::IndexForTextMarker,
        )?;
        let end_index = Self::marker_number(
            element,
            "AXIndexForTextMarker",
            &end,
            AxStage::IndexForTextMarker,
        )?;
        let reported_length = Self::marker_number(
            element,
            "AXLengthForTextMarkerRange",
            marker_range,
            AxStage::LengthForTextMarkerRange,
        )?;
        let calculated_length = end_index
            .checked_sub(start_index)
            .filter(|length| *length > 0 && *length == reported_length)
            .ok_or(VerbalixError::SelectionUnavailable)?;
        let location =
            isize::try_from(start_index).map_err(|_| VerbalixError::SelectionUnavailable)?;
        let length =
            isize::try_from(calculated_length).map_err(|_| VerbalixError::SelectionUnavailable)?;
        crate::diagnostics::ax_resolution(AxStage::IndexForTextMarker, origin, AxCategory::Success);
        Ok(CFRange { location, length })
    }

    fn marker_number(
        element: AXUIElementRef,
        attribute: &str,
        parameter: &OwnedCfValue,
        stage: AxStage,
    ) -> Result<i64, VerbalixError> {
        let origin = ExtractionOrigin::TextMarker;
        let value =
            Self::parameterized_attribute(element, attribute, parameter.as_ref(), stage, origin)
                .map_err(|_| VerbalixError::SelectionUnavailable)?;
        if unsafe { CFGetTypeID(value.as_ref()) != CFNumberGetTypeID() } {
            crate::diagnostics::ax_resolution(stage, origin, AxCategory::UnexpectedType);
            return Err(VerbalixError::SelectionUnavailable);
        }
        let number = unsafe { CFNumber::wrap_under_get_rule(value.as_ref().cast()) };
        number
            .to_i64()
            .filter(|number| *number >= 0)
            .ok_or(VerbalixError::SelectionUnavailable)
    }

    fn extract(element: AXUIElementRef) -> Result<ExtractedSelection, VerbalixError> {
        let direct = Self::string_attribute_at(
            element,
            "AXSelectedText",
            AxStage::SelectedText,
            ExtractionOrigin::SelectedText,
        );
        match direct {
            Ok(text) => Self::direct_selection(element, text),
            Err(failure) if marker_fallback(failure.category) => {
                Self::cf_range_selection(element).or_else(|_| Self::marker_selection(element))
            }
            Err(_) => Err(VerbalixError::SelectionUnavailable),
        }
    }

    fn capture_from_element(element: &OwnedAxElement) -> Result<SelectionSnapshot, VerbalixError> {
        let origin = ExtractionOrigin::SelectedText;
        let pid = Self::validated_pid(element.as_ref(), origin)?;
        let role = Self::string_attribute_at(element.as_ref(), "AXRole", AxStage::Role, origin)
            .map_err(|_| VerbalixError::SelectionUnavailable)?;
        if role == "AXSecureTextField" {
            return Err(VerbalixError::ProtectedField);
        }
        let extracted = Self::extract(element.as_ref())?;
        if extracted.text.trim().is_empty() {
            return Err(VerbalixError::SelectionUnavailable);
        }
        if extracted.text.chars().count() > 12_000 {
            return Err(VerbalixError::TextTooLong);
        }
        if Self::validated_pid(element.as_ref(), origin)? != pid {
            return Err(VerbalixError::StaleSelection);
        }
        let current_role =
            Self::string_attribute_at(element.as_ref(), "AXRole", AxStage::Role, origin)
                .map_err(|_| VerbalixError::SelectionUnavailable)?;
        if current_role != role {
            return Err(VerbalixError::StaleSelection);
        }
        Ok(SelectionSnapshot::new(
            pid,
            format!("pid:{pid}"),
            extracted.text,
            TextRange {
                location: extracted.range.location as i64,
                length: extracted.range.length as i64,
            },
            extracted.bounds,
            extracted.writable,
        )
        .with_geometry_source(extracted.geometry_source))
    }

    fn validated_pid(
        element: AXUIElementRef,
        origin: ExtractionOrigin,
    ) -> Result<i32, VerbalixError> {
        let mut pid = 0;
        let status = unsafe { AXUIElementGetPid(element, &mut pid) };
        if status != AX_SUCCESS || pid <= 0 {
            crate::diagnostics::ax_resolution(AxStage::Pid, origin, AxCategory::InvalidPid);
            return Err(VerbalixError::SelectionUnavailable);
        }
        let own_pid =
            i32::try_from(std::process::id()).map_err(|_| VerbalixError::SelectionUnavailable)?;
        if pid == own_pid {
            crate::diagnostics::ax_resolution(AxStage::Pid, origin, AxCategory::SelfProcess);
            return Err(VerbalixError::SelectionUnavailable);
        }
        crate::diagnostics::ax_resolution(AxStage::Pid, origin, AxCategory::Success);
        Ok(pid)
    }

    pub(super) fn writable(element: AXUIElementRef) -> bool {
        let attribute = CFString::new("AXSelectedText");
        let mut settable: Boolean = 0;
        unsafe {
            AXUIElementIsAttributeSettable(element, attribute.as_concrete_TypeRef(), &mut settable)
                == AX_SUCCESS
                && settable != 0
        }
    }

    fn writable_at(element: AXUIElementRef, origin: ExtractionOrigin) -> bool {
        let writable = Self::writable(element);
        crate::diagnostics::ax_resolution(
            AxStage::SelectedTextSettable,
            origin,
            if writable {
                AxCategory::Settable
            } else {
                AxCategory::NotSettable
            },
        );
        writable
    }
}

impl SelectionPort for MacAccessibility {
    fn permission_granted(&self, prompt: bool) -> bool {
        unsafe {
            if !prompt {
                return AXIsProcessTrusted() != 0;
            }
            let prompt_key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let options = CFDictionary::from_CFType_pairs(&[(prompt_key, CFBoolean::true_value())]);
            AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0
        }
    }

    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        if !self.permission_granted(false) {
            crate::diagnostics::ax_resolution(
                AxStage::Trust,
                ExtractionOrigin::SelectedText,
                AxCategory::ApiDisabled,
            );
            return Err(VerbalixError::PermissionDenied);
        }
        crate::diagnostics::ax_resolution(
            AxStage::Trust,
            ExtractionOrigin::SelectedText,
            AxCategory::Success,
        );
        let element = Self::focused_element()?;
        Self::capture_from_element(&element)
    }

    fn replace(&self, expected: &SelectionSnapshot, text: &str) -> Result<(), VerbalixError> {
        let element = Self::focused_element()?;
        let current = Self::capture_from_element(&element)?;
        if !current.same_target(expected) || !current.writable {
            return Err(VerbalixError::StaleSelection);
        }
        let value = CFString::new(text);
        let attribute = CFString::new("AXSelectedText");
        let status = unsafe {
            AXUIElementSetAttributeValue(
                element.as_ref(),
                attribute.as_concrete_TypeRef(),
                value.as_CFTypeRef(),
            )
        };
        (status == AX_SUCCESS)
            .then_some(())
            .ok_or(VerbalixError::LocalFailure)
    }

    fn restore(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
    ) -> Result<(), VerbalixError> {
        super::macos_restore::restore(expected, transformed_text)
    }
}
