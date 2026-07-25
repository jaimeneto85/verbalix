use super::{
    macos_attribute, macos_ax, macos_selection, macos_selection_revalidation, macos_text_role,
    macos_value_range,
};
use crate::{
    application::TransformLease,
    domain::{SelectionElementIdentity, SelectionSnapshot, VerbalixError},
};

pub fn restore(expected: &SelectionSnapshot, transformed_text: &str) -> Result<(), VerbalixError> {
    restore_validated(expected, transformed_text, || true)
}

pub fn restore_guarded(
    expected: &SelectionSnapshot,
    transformed_text: &str,
    lease: &TransformLease,
) -> Result<(), VerbalixError> {
    restore_validated(expected, transformed_text, || lease.try_claim_write())
}

fn restore_validated(
    expected: &SelectionSnapshot,
    transformed_text: &str,
    claim: impl FnOnce() -> bool,
) -> Result<(), VerbalixError> {
    let expected_identity = expected_strong_identity(expected)?;
    let element = macos_ax::focused_element().map_err(|_| VerbalixError::StaleSelection)?;
    let pid = macos_ax::pid(element.as_ref()).map_err(|_| VerbalixError::StaleSelection)?;
    let own_pid = i32::try_from(std::process::id()).map_err(|_| VerbalixError::StaleSelection)?;
    let role = macos_selection::role(element.as_ref())?;
    let _capability = macos_text_role::validate(&role).map_err(|error| match error {
        VerbalixError::ProtectedField => error,
        _ => VerbalixError::StaleSelection,
    })?;
    let current_identity = macos_selection::element_identity(element.as_ref(), role.clone())?;
    validate_restore_target(
        expected,
        expected_identity,
        pid,
        own_pid,
        &role,
        macos_attribute::selected_text_writable(element.as_ref())
            .map_err(|_| VerbalixError::StaleSelection)?,
        &current_identity,
    )?;
    if expected.extraction_strategy == crate::domain::SelectionExtractionStrategy::ValueRange
        && !macos_value_range::role_eligible(&role)
    {
        return Err(VerbalixError::StaleSelection);
    }
    let current =
        macos_selection_revalidation::read(element.as_ref(), expected.extraction_strategy)?;
    validate_restore_selection(expected, transformed_text, &current)?;
    if !claim() {
        return Err(VerbalixError::StaleSelection);
    }
    macos_ax::set_selected_text(element.as_ref(), &expected.text)
        .then_some(())
        .ok_or(VerbalixError::LocalFailure)
}

fn validate_restore_target(
    expected: &SelectionSnapshot,
    expected_identity: &SelectionElementIdentity,
    pid: i32,
    own_pid: i32,
    role: &str,
    writable: bool,
    current_identity: &SelectionElementIdentity,
) -> Result<(), VerbalixError> {
    if role == "AXSecureTextField" {
        return Err(VerbalixError::ProtectedField);
    }
    if !expected.writable
        || pid <= 0
        || pid == own_pid
        || pid != expected.pid
        || role.is_empty()
        || !writable
        || !same_strong_identity(expected_identity, current_identity)
    {
        return Err(VerbalixError::StaleSelection);
    }
    Ok(())
}

fn expected_strong_identity(
    expected: &SelectionSnapshot,
) -> Result<&SelectionElementIdentity, VerbalixError> {
    let identity = expected
        .writable
        .then_some(expected.element_identity.as_ref())
        .flatten()
        .ok_or(VerbalixError::StaleSelection)?;
    identity
        .strong_identifier()
        .is_some()
        .then_some(identity)
        .ok_or(VerbalixError::StaleSelection)
}

fn same_strong_identity(
    expected: &SelectionElementIdentity,
    current: &SelectionElementIdentity,
) -> bool {
    match (expected.strong_identifier(), current.strong_identifier()) {
        (Some(expected_identifier), Some(current_identifier)) => {
            expected_identifier == current_identifier && expected == current
        }
        _ => false,
    }
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
