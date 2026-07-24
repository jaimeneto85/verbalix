use super::{macos_ax, macos_selection};
use crate::{
    application::TransformLease,
    domain::{SelectionSnapshot, VerbalixError},
};

pub fn replace(expected: &SelectionSnapshot, text: &str) -> Result<(), VerbalixError> {
    replace_validated(expected, text, || true)
}

pub fn replace_guarded(
    expected: &SelectionSnapshot,
    text: &str,
    lease: &TransformLease,
) -> Result<(), VerbalixError> {
    replace_validated(expected, text, || lease.try_claim_write())
}

fn replace_validated(
    expected: &SelectionSnapshot,
    text: &str,
    claim: impl FnOnce() -> bool,
) -> Result<(), VerbalixError> {
    validate_expected(expected)?;
    let element = macos_ax::focused_element_for_pid(expected.pid)
        .map_err(|_| VerbalixError::StaleSelection)?;
    let current = macos_selection::capture_with_strategy(&element, expected.extraction_strategy)?;
    validate_current(expected, &current)?;
    if !claim() {
        return Err(VerbalixError::StaleSelection);
    }
    macos_ax::set_selected_text(element.as_ref(), text)
        .then_some(())
        .ok_or(VerbalixError::LocalFailure)
}

fn validate_expected(expected: &SelectionSnapshot) -> Result<(), VerbalixError> {
    let strong = expected
        .element_identity
        .as_ref()
        .and_then(|identity| identity.strong_identifier());
    if !expected.writable || expected.pid <= 0 || strong.is_none() {
        return Err(VerbalixError::StaleSelection);
    }
    Ok(())
}

fn validate_current(
    expected: &SelectionSnapshot,
    current: &SelectionSnapshot,
) -> Result<(), VerbalixError> {
    if current.writable && current.same_target(expected) {
        Ok(())
    } else {
        Err(VerbalixError::StaleSelection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Rect, SelectionElementIdentity, TextRange};

    fn snapshot(identifier: Option<&str>, writable: bool) -> SelectionSnapshot {
        SelectionSnapshot::new(
            42,
            "pid:42".to_owned(),
            "same".to_owned(),
            TextRange {
                location: 3,
                length: 4,
            },
            Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            },
            writable,
        )
        .with_element_identity(SelectionElementIdentity {
            role: "AXTextArea".to_owned(),
            subrole: None,
            identifier: identifier.map(str::to_owned),
            frame: Rect {
                x: 1.0,
                y: 2.0,
                width: 30.0,
                height: 40.0,
            },
        })
    }

    #[test]
    fn expected_target_requires_writable_positive_pid_and_strong_identifier() {
        let mut invalid_pid = snapshot(Some("editor"), true);
        invalid_pid.pid = 0;
        for invalid in [
            snapshot(Some("editor"), false),
            snapshot(None, true),
            snapshot(Some(""), true),
            snapshot(Some("  "), true),
            invalid_pid,
        ] {
            assert!(matches!(
                validate_expected(&invalid),
                Err(VerbalixError::StaleSelection)
            ));
        }
        assert!(validate_expected(&snapshot(Some("editor"), true)).is_ok());
    }

    #[test]
    fn current_target_must_match_every_identity_and_selection_field() {
        let expected = snapshot(Some("editor"), true);
        assert!(validate_current(&expected, &expected).is_ok());
        let mut changed = expected.clone();
        changed.id = uuid::Uuid::new_v4();
        assert!(validate_current(&expected, &changed).is_ok());
        changed.range.location += 1;
        assert!(matches!(
            validate_current(&expected, &changed),
            Err(VerbalixError::StaleSelection)
        ));
        let mut read_only = expected.clone();
        read_only.writable = false;
        assert!(matches!(
            validate_current(&expected, &read_only),
            Err(VerbalixError::StaleSelection)
        ));
    }

    #[test]
    fn unicode_text_and_utf16_range_must_still_match_the_original_target() {
        let mut expected = snapshot(Some("editor"), true);
        expected.text = "Olá 👩🏽‍💻".to_owned();
        expected.range.length = expected.text.encode_utf16().count() as i64;
        assert!(validate_current(&expected, &expected).is_ok());

        let mut changed_text = expected.clone();
        changed_text.text = "Olá 👩🏽‍🔧".to_owned();
        let mut changed_length = expected.clone();
        changed_length.range.length -= 1;
        let mut changed_pid = expected.clone();
        changed_pid.pid += 1;
        let mut changed_identity = expected.clone();
        changed_identity
            .element_identity
            .as_mut()
            .unwrap()
            .identifier = Some("another-editor".to_owned());

        for changed in [changed_text, changed_length, changed_pid, changed_identity] {
            assert!(matches!(
                validate_current(&expected, &changed),
                Err(VerbalixError::StaleSelection)
            ));
        }
    }
}
