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

#[test]
fn restore_rejects_another_field_in_the_same_pid_even_with_matching_selection() {
    let expected = snapshot(true);
    let transformed = "A👩🏽‍💻";
    let matching_selection = macos_selection::ClassicSelection {
        text: transformed.to_owned(),
        range: CFRange {
            location: expected.range.location as isize,
            length: transformed.encode_utf16().count() as isize,
        },
    };

    assert!(validate_restore_selection(&expected, transformed, &matching_selection).is_ok());
    assert!(matches!(
        validate_restore_target(
            &expected,
            expected.element_identity.as_ref().unwrap(),
            expected.pid,
            7,
            "AXTextArea",
            true,
            &identity("another-editor"),
        ),
        Err(VerbalixError::StaleSelection)
    ));
}

#[test]
fn restore_without_identifier_rejects_before_the_write_boundary() {
    let transformed = "A👩🏽‍💻";

    for identifier in [None, Some(String::new()), Some("  ".to_owned())] {
        let mut expected = snapshot(true);
        expected.element_identity.as_mut().unwrap().identifier = identifier;
        let weak_identity = expected.element_identity.as_ref().unwrap();
        let matching_selection = macos_selection::ClassicSelection {
            text: transformed.to_owned(),
            range: CFRange {
                location: expected.range.location as isize,
                length: transformed.encode_utf16().count() as isize,
            },
        };
        let mut writes = 0;

        let validation = expected_strong_identity(&expected)
            .and_then(|identity| {
                validate_restore_target(
                    &expected,
                    identity,
                    expected.pid,
                    7,
                    "AXTextArea",
                    true,
                    weak_identity,
                )
            })
            .and_then(|_| validate_restore_selection(&expected, transformed, &matching_selection));
        if validation.is_ok() {
            writes += 1;
        }

        assert!(matches!(validation, Err(VerbalixError::StaleSelection)));
        assert_eq!(writes, 0);
        assert!(validate_restore_selection(&expected, transformed, &matching_selection).is_ok());
    }
}
