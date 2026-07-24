use super::{macos_ax, macos_selection};
use crate::domain::{SelectionElementIdentity, SelectionSnapshot, VerbalixError};

pub fn restore(expected: &SelectionSnapshot, transformed_text: &str) -> Result<(), VerbalixError> {
    let expected_identity = expected
        .writable
        .then_some(expected.element_identity.as_ref())
        .flatten()
        .ok_or(VerbalixError::StaleSelection)?;
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
        || current_identity != expected_identity
    {
        return Err(VerbalixError::StaleSelection);
    }
    Ok(())
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
mod tests {
    use super::*;
    use crate::{
        domain::{Rect, TextRange},
        platform::macos_classic_range::CFRange,
    };

    fn identity(identifier: &str) -> SelectionElementIdentity {
        SelectionElementIdentity {
            role: "AXTextArea".to_owned(),
            subrole: None,
            identifier: Some(identifier.to_owned()),
            frame: Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            },
        }
    }

    fn snapshot(writable: bool) -> SelectionSnapshot {
        SelectionSnapshot::new(
            42,
            "pid:42".to_owned(),
            "original".to_owned(),
            TextRange {
                location: 3,
                length: 8,
            },
            Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            },
            writable,
        )
        .with_element_identity(identity("editor"))
    }

    #[test]
    fn marker_snapshot_never_reaches_restore_mutation() {
        let expected = snapshot(false);
        assert!(matches!(
            validate_restore_target(
                &expected,
                expected.element_identity.as_ref().unwrap(),
                42,
                7,
                "AXTextArea",
                true,
                expected.element_identity.as_ref().unwrap(),
            ),
            Err(VerbalixError::StaleSelection)
        ));
    }

    #[test]
    fn restore_rejects_self_wrong_pid_secure_read_only_and_changed_element() {
        let expected = snapshot(true);
        let expected_identity = expected.element_identity.as_ref().unwrap();

        for (pid, own_pid, role, writable, current_identity) in [
            (42, 42, "AXTextArea", true, identity("editor")),
            (43, 7, "AXTextArea", true, identity("editor")),
            (42, 7, "AXTextArea", false, identity("editor")),
            (42, 7, "AXTextArea", true, identity("another-editor")),
        ] {
            assert!(matches!(
                validate_restore_target(
                    &expected,
                    expected_identity,
                    pid,
                    own_pid,
                    role,
                    writable,
                    &current_identity,
                ),
                Err(VerbalixError::StaleSelection)
            ));
        }
        assert!(matches!(
            validate_restore_target(
                &expected,
                expected_identity,
                42,
                7,
                "AXSecureTextField",
                true,
                expected_identity,
            ),
            Err(VerbalixError::ProtectedField)
        ));
    }

    #[test]
    fn restore_accepts_only_the_current_transformed_selection_and_utf16_range() {
        let expected = snapshot(true);
        let transformed = "A👩🏽‍💻";
        let current = macos_selection::ClassicSelection {
            text: transformed.to_owned(),
            range: CFRange {
                location: 3,
                length: transformed.encode_utf16().count() as isize,
            },
        };

        assert!(validate_restore_selection(&expected, transformed, &current).is_ok());

        for stale in [
            macos_selection::ClassicSelection {
                text: "another".to_owned(),
                range: current.range,
            },
            macos_selection::ClassicSelection {
                text: transformed.to_owned(),
                range: CFRange {
                    location: 4,
                    ..current.range
                },
            },
            macos_selection::ClassicSelection {
                text: transformed.to_owned(),
                range: CFRange {
                    length: current.range.length - 1,
                    ..current.range
                },
            },
        ] {
            assert!(matches!(
                validate_restore_selection(&expected, transformed, &stale),
                Err(VerbalixError::StaleSelection)
            ));
        }
    }
}
