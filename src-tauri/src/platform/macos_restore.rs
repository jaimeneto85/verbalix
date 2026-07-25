use super::{
    macos_attribute,
    macos_ax::{self, AXUIElementRef, AxWriteResult, OwnedAxElement},
    macos_selection, macos_selection_revalidation, macos_value_range,
};
use crate::domain::{SelectionElementIdentity, SelectionSnapshot, VerbalixError};

pub(super) enum RestoreWriteOutcome {
    Confirmed,
    Rejected,
    Indeterminate,
}

pub(super) fn prepare_on_element(
    expected: &SelectionSnapshot,
    transformed_text: &str,
    element: &OwnedAxElement,
    causal: bool,
) -> Result<(), VerbalixError> {
    let expected_identity = expected_identity(expected, causal)?;
    let pid = macos_ax::pid(element.as_ref()).map_err(|_| VerbalixError::StaleSelection)?;
    let own_pid = i32::try_from(std::process::id()).map_err(|_| VerbalixError::StaleSelection)?;
    let text_role = macos_selection::text_role(element.as_ref()).map_err(|error| match error {
        VerbalixError::ProtectedField => error,
        _ => VerbalixError::StaleSelection,
    })?;
    let current_identity = macos_selection::element_identity(element.as_ref(), &text_role)?;
    let current_identifier = macos_selection::native_identifier(element.as_ref())?;
    validate_restore_target_with_causality(
        expected,
        RestoreTarget {
            expected_identity,
            pid,
            own_pid,
            role: &text_role.role,
            writable: macos_attribute::selected_text_writable(element.as_ref())
                .map_err(|_| VerbalixError::StaleSelection)?,
            current_identity: &current_identity,
            expected_identifier: expected.native_element_identifier(),
            current_identifier: current_identifier.as_deref(),
            causal,
        },
    )?;
    if expected.extraction_strategy == crate::domain::SelectionExtractionStrategy::ValueRange
        && !macos_value_range::role_eligible(&text_role.role)
    {
        return Err(VerbalixError::StaleSelection);
    }
    let current =
        macos_selection_revalidation::read(element.as_ref(), expected.extraction_strategy)?;
    validate_restore_selection(expected, transformed_text, &current)
}

pub(super) fn write_on_element(
    expected: &SelectionSnapshot,
    element: AXUIElementRef,
) -> RestoreWriteOutcome {
    match macos_ax::set_selected_text(element, &expected.text) {
        AxWriteResult::Confirmed => RestoreWriteOutcome::Confirmed,
        AxWriteResult::Rejected(_) => RestoreWriteOutcome::Rejected,
        AxWriteResult::Indeterminate(_) => {
            let Ok(current) =
                macos_selection_revalidation::read(element, expected.extraction_strategy)
            else {
                return RestoreWriteOutcome::Indeterminate;
            };
            let Ok(location) = isize::try_from(expected.range.location) else {
                return RestoreWriteOutcome::Indeterminate;
            };
            let Ok(length) = isize::try_from(expected.text.encode_utf16().count()) else {
                return RestoreWriteOutcome::Indeterminate;
            };
            if current.text == expected.text
                && current.range.location == location
                && current.range.length == length
            {
                RestoreWriteOutcome::Confirmed
            } else {
                RestoreWriteOutcome::Indeterminate
            }
        }
    }
}

struct RestoreTarget<'a> {
    expected_identity: &'a SelectionElementIdentity,
    pid: i32,
    own_pid: i32,
    role: &'a str,
    writable: bool,
    current_identity: &'a SelectionElementIdentity,
    expected_identifier: Option<&'a str>,
    current_identifier: Option<&'a str>,
    causal: bool,
}

fn validate_restore_target_with_causality(
    expected: &SelectionSnapshot,
    target: RestoreTarget<'_>,
) -> Result<(), VerbalixError> {
    if target.role == "AXSecureTextField" {
        return Err(VerbalixError::ProtectedField);
    }
    if !expected.writable
        || target.pid <= 0
        || target.pid == target.own_pid
        || target.pid != expected.pid
        || target.role.is_empty()
        || !target.writable
        || if target.causal {
            target.expected_identity != target.current_identity
        } else {
            target.expected_identifier != target.current_identifier
                || target.expected_identifier.is_none()
                || target.expected_identity != target.current_identity
        }
    {
        return Err(VerbalixError::StaleSelection);
    }
    Ok(())
}

#[cfg(test)]
fn validate_restore_target(
    expected: &SelectionSnapshot,
    expected_identity: &SelectionElementIdentity,
    pid: i32,
    own_pid: i32,
    role: &str,
    writable: bool,
    current_identity: &SelectionElementIdentity,
    current_identifier: Option<&str>,
) -> Result<(), VerbalixError> {
    validate_restore_target_with_causality(
        expected,
        RestoreTarget {
            expected_identity,
            pid,
            own_pid,
            role,
            writable,
            current_identity,
            expected_identifier: expected.native_element_identifier(),
            current_identifier,
            causal: false,
        },
    )
}

fn expected_identity(
    expected: &SelectionSnapshot,
    causal: bool,
) -> Result<&SelectionElementIdentity, VerbalixError> {
    let identity = expected
        .writable
        .then_some(expected.element_identity.as_ref())
        .flatten()
        .ok_or(VerbalixError::StaleSelection)?;
    if causal || expected.native_element_identifier().is_some() {
        Ok(identity)
    } else {
        Err(VerbalixError::StaleSelection)
    }
}

#[cfg(test)]
fn expected_strong_identity(
    expected: &SelectionSnapshot,
) -> Result<&SelectionElementIdentity, VerbalixError> {
    expected_identity(expected, false)
}

fn validate_restore_selection(
    expected: &SelectionSnapshot,
    transformed_text: &str,
    current: &macos_selection_revalidation::CurrentSelection,
) -> Result<(), VerbalixError> {
    let expected_location =
        isize::try_from(expected.range.location).map_err(|_| VerbalixError::StaleSelection)?;
    let transformed_length = isize::try_from(transformed_text.encode_utf16().count())
        .map_err(|_| VerbalixError::StaleSelection)?;
    if current.text != transformed_text
        || current.range.location != expected_location
        || current.range.length != transformed_length
        || current.strategy != expected.extraction_strategy
    {
        return Err(VerbalixError::StaleSelection);
    }
    Ok(())
}

#[cfg(test)]
#[path = "macos_restore_tests.rs"]
mod tests;
