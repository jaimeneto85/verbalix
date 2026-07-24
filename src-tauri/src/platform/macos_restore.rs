use super::{macos_ax, macos_selection};
use crate::domain::{SelectionElementIdentity, SelectionSnapshot, VerbalixError};

pub fn restore(expected: &SelectionSnapshot, transformed_text: &str) -> Result<(), VerbalixError> {
    let expected_identity = expected_strong_identity(expected)?;
    let element = macos_ax::focused_element().map_err(|_| VerbalixError::StaleSelection)?;
    let pid = macos_ax::pid(element.as_ref()).map_err(|_| VerbalixError::StaleSelection)?;
    let own_pid = i32::try_from(std::process::id()).map_err(|_| VerbalixError::StaleSelection)?;
    let role = macos_selection::role(element.as_ref())?;
    if role == "AXSecureTextField" {
        return Err(VerbalixError::ProtectedField);
    }
    let current_identity = macos_selection::element_identity(element.as_ref(), role.clone())?;
    validate_restore_target(
        expected,
        expected_identity,
        pid,
        own_pid,
        &role,
        macos_ax::writable(element.as_ref()),
        &current_identity,
    )?;
    let current = macos_selection::classic_selection(element.as_ref())?;
    validate_restore_selection(expected, transformed_text, &current)?;
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
    strong_identifier(identity)
        .is_some()
        .then_some(identity)
        .ok_or(VerbalixError::StaleSelection)
}

fn same_strong_identity(
    expected: &SelectionElementIdentity,
    current: &SelectionElementIdentity,
) -> bool {
    match (strong_identifier(expected), strong_identifier(current)) {
        (Some(expected_identifier), Some(current_identifier)) => {
            expected_identifier == current_identifier && expected == current
        }
        _ => false,
    }
}

fn strong_identifier(identity: &SelectionElementIdentity) -> Option<&str> {
    identity
        .identifier
        .as_deref()
        .filter(|identifier| !identifier.trim().is_empty())
}

fn validate_restore_selection(
    expected: &SelectionSnapshot,
    transformed_text: &str,
    current: &macos_selection::ClassicSelection,
) -> Result<(), VerbalixError> {
    let expected_location =
        isize::try_from(expected.range.location).map_err(|_| VerbalixError::StaleSelection)?;
    let transformed_length = isize::try_from(transformed_text.encode_utf16().count())
        .map_err(|_| VerbalixError::StaleSelection)?;
    if current.text != transformed_text
        || current.range.location != expected_location
        || current.range.length != transformed_length
    {
        return Err(VerbalixError::StaleSelection);
    }
    Ok(())
}

#[cfg(test)]
#[path = "macos_restore_tests.rs"]
mod tests;
