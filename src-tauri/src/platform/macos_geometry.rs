use super::macos_ax::AXUIElementRef;
use crate::domain::{GeometrySource, Rect};
use core_foundation::{
    base::{CFGetTypeID, CFRelease, CFTypeRef, TCFType},
    string::{CFString, CFStringRef},
};
use std::{ffi::c_void, ptr};

type AXValueRef = *const c_void;
type CGEventRef = *const c_void;
type AXError = i32;

const AX_SUCCESS: AXError = 0;
const AX_VALUE_CF_RANGE: i32 = 4;
const AX_VALUE_CG_POINT: i32 = 1;
const AX_VALUE_CG_SIZE: i32 = 2;
const AX_VALUE_CG_RECT: i32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CFRange {
    location: isize,
    length: isize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
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
    fn AXValueCreate(value_type: i32, value: *const c_void) -> AXValueRef;
    fn AXValueGetType(value: AXValueRef) -> i32;
    fn AXValueGetTypeID() -> usize;
    fn AXValueGetValue(value: AXValueRef, value_type: i32, output: *mut c_void) -> u8;
    fn CGEventCreate(source: *const c_void) -> CGEventRef;
    fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
}

pub(super) fn resolve(
    element: AXUIElementRef,
    range_location: isize,
    range_length: isize,
) -> Option<(Rect, GeometrySource)> {
    let selected_range = selected_range_bounds(element, range_location, range_length);
    let focused_element = focused_element_frame(element);
    let cursor = cursor_position();
    select_geometry(selected_range, focused_element, cursor)
}

fn selected_range_bounds(element: AXUIElementRef, location: isize, length: isize) -> Option<Rect> {
    let range = CFRange { location, length };
    let parameter = unsafe { AXValueCreate(AX_VALUE_CF_RANGE, (&range as *const CFRange).cast()) };
    if parameter.is_null() {
        return None;
    }
    let attribute = CFString::new("AXBoundsForRange");
    let mut value = ptr::null();
    let status = unsafe {
        AXUIElementCopyParameterizedAttributeValue(
            element,
            attribute.as_concrete_TypeRef(),
            parameter,
            &mut value,
        )
    };
    unsafe { CFRelease(parameter) };
    if status != AX_SUCCESS {
        release(value);
        return None;
    }
    read_rect(value)
}

fn focused_element_frame(element: AXUIElementRef) -> Option<Rect> {
    if let Some(frame) = copy_attribute(element, "AXFrame")
        .and_then(read_rect)
        .filter(valid_frame)
    {
        return Some(frame);
    }
    let position = copy_attribute(element, "AXPosition").and_then(read_point)?;
    let size = copy_attribute(element, "AXSize").and_then(read_size)?;
    Some(Rect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}

pub(super) fn element_frame(element: AXUIElementRef) -> Option<Rect> {
    focused_element_frame(element)
}

fn cursor_position() -> Option<CGPoint> {
    let event = unsafe { CGEventCreate(ptr::null()) };
    if event.is_null() {
        return None;
    }
    let point = unsafe { CGEventGetLocation(event) };
    unsafe { CFRelease(event) };
    point
        .x
        .is_finite()
        .then_some(point)
        .filter(|point| point.y.is_finite())
}

fn copy_attribute(element: AXUIElementRef, name: &str) -> Option<CFTypeRef> {
    let attribute = CFString::new(name);
    let mut value = ptr::null();
    let status = unsafe {
        AXUIElementCopyAttributeValue(element, attribute.as_concrete_TypeRef(), &mut value)
    };
    if status == AX_SUCCESS && !value.is_null() {
        Some(value)
    } else {
        release(value);
        None
    }
}

fn read_rect(value: CFTypeRef) -> Option<Rect> {
    let rect = rect_from_value(value);
    release(value);
    rect
}

pub(super) fn rect_from_value(value: CFTypeRef) -> Option<Rect> {
    if !has_ax_value_type(value, AX_VALUE_CG_RECT) {
        return None;
    }
    let mut rect = CGRect::default();
    let success =
        unsafe { AXValueGetValue(value, AX_VALUE_CG_RECT, (&mut rect as *mut CGRect).cast()) };
    (success != 0)
        .then_some(Rect {
            x: rect.origin.x,
            y: rect.origin.y,
            width: rect.size.width,
            height: rect.size.height,
        })
        .filter(valid_frame)
}

fn read_point(value: CFTypeRef) -> Option<CGPoint> {
    if !has_ax_value_type(value, AX_VALUE_CG_POINT) {
        release(value);
        return None;
    }
    let mut point = CGPoint::default();
    let success = unsafe {
        AXValueGetValue(
            value,
            AX_VALUE_CG_POINT,
            (&mut point as *mut CGPoint).cast(),
        )
    };
    release(value);
    (success != 0).then_some(point)
}

fn read_size(value: CFTypeRef) -> Option<CGSize> {
    if !has_ax_value_type(value, AX_VALUE_CG_SIZE) {
        release(value);
        return None;
    }
    let mut size = CGSize::default();
    let success =
        unsafe { AXValueGetValue(value, AX_VALUE_CG_SIZE, (&mut size as *mut CGSize).cast()) };
    release(value);
    (success != 0).then_some(size)
}

fn has_ax_value_type(value: CFTypeRef, expected_type: i32) -> bool {
    if value.is_null() {
        return false;
    }
    let is_ax_value = unsafe { CFGetTypeID(value) == AXValueGetTypeID() };
    is_ax_value
        && ax_value_type_matches(is_ax_value, unsafe { AXValueGetType(value) }, expected_type)
}

fn ax_value_type_matches(is_ax_value: bool, actual_type: i32, expected_type: i32) -> bool {
    is_ax_value && actual_type == expected_type
}

fn release(value: CFTypeRef) {
    if !value.is_null() {
        unsafe { CFRelease(value) };
    }
}

fn select_geometry(
    selected_range: Option<Rect>,
    focused_element: Option<Rect>,
    cursor: Option<CGPoint>,
) -> Option<(Rect, GeometrySource)> {
    if let Some(bounds) = selected_range.filter(valid_selected_range) {
        return Some((bounds, GeometrySource::SelectedRange));
    }
    if let Some(bounds) = focused_element.filter(valid_frame) {
        return Some((bounds, GeometrySource::FocusedElement));
    }
    cursor
        .filter(|point| point.x.is_finite() && point.y.is_finite())
        .map(|point| {
            (
                Rect {
                    x: point.x,
                    y: point.y,
                    width: 1.0,
                    height: 1.0,
                },
                GeometrySource::Cursor,
            )
        })
}

fn finite(bounds: &Rect) -> bool {
    bounds.x.is_finite()
        && bounds.y.is_finite()
        && bounds.width.is_finite()
        && bounds.height.is_finite()
}

fn valid_selected_range(bounds: &Rect) -> bool {
    finite(bounds) && bounds.width > 1.0 && bounds.height > 1.0
}

fn valid_frame(bounds: &Rect) -> bool {
    finite(bounds) && bounds.width > 0.0 && bounds.height > 0.0
}

#[cfg(test)]
#[path = "macos_geometry_tests.rs"]
mod tests;
