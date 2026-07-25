use super::{
    macos_ax::{AXUIElementRef, AxWriteResult, OwnedAxElement},
    macos_selection, macos_selection_revalidation, macos_write_boundary,
};
use crate::domain::{SelectionSnapshot, VerbalixError};

pub(super) enum WriteOutcome {
    Confirmed,
    Rejected,
    Indeterminate,
}

pub(super) fn prepare_on_element(
    expected: &SelectionSnapshot,
    element: &OwnedAxElement,
    causal: bool,
) -> Result<(), VerbalixError> {
    validate_expected(expected, causal)?;
    let current = macos_selection::capture_with_strategy(element, expected.extraction_strategy)?;
    validate_current(expected, &current)
}

pub(super) fn write_on_element(
    expected: &SelectionSnapshot,
    text: &str,
    element: AXUIElementRef,
) -> WriteOutcome {
    match macos_write_boundary::set_selected_text(expected, text, element) {
        Err(_) => WriteOutcome::Rejected,
        Ok(AxWriteResult::Confirmed) => WriteOutcome::Confirmed,
        Ok(AxWriteResult::Rejected(_)) => WriteOutcome::Rejected,
        Ok(AxWriteResult::Indeterminate(_)) => {
            let Ok(current) =
                macos_selection_revalidation::read(element, expected.extraction_strategy)
            else {
                return WriteOutcome::Indeterminate;
            };
            let Ok(location) = isize::try_from(expected.range.location) else {
                return WriteOutcome::Indeterminate;
            };
            let Ok(transformed_length) = isize::try_from(text.encode_utf16().count()) else {
                return WriteOutcome::Indeterminate;
            };
            if current.text == text
                && current.range.location == location
                && current.range.length == transformed_length
            {
                WriteOutcome::Confirmed
            } else if current.text == expected.text
                && current.range.location == location
                && current.range.length == expected.range.length as isize
            {
                WriteOutcome::Rejected
            } else {
                WriteOutcome::Indeterminate
            }
        }
    }
}

fn validate_expected(expected: &SelectionSnapshot, causal: bool) -> Result<(), VerbalixError> {
    if !expected.writable
        || expected.pid <= 0
        || (expected.native_element_identifier().is_none() && !causal)
    {
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
            frame: Rect {
                x: 1.0,
                y: 2.0,
                width: 30.0,
                height: 40.0,
            },
        })
        .with_native_element_identifier(identifier.map(str::to_owned))
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
                validate_expected(&invalid, false),
                Err(VerbalixError::StaleSelection)
            ));
        }
        assert!(validate_expected(&snapshot(Some("editor"), true), false).is_ok());
        assert!(validate_expected(&snapshot(None, true), true).is_ok());
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
        let mut another_strategy = expected.clone();
        another_strategy.extraction_strategy =
            crate::domain::SelectionExtractionStrategy::ValueRange;
        assert!(validate_current(&expected, &another_strategy).is_err());
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
        changed_identity =
            changed_identity.with_native_element_identifier(Some("another-editor".to_owned()));

        for changed in [changed_text, changed_length, changed_pid, changed_identity] {
            assert!(matches!(
                validate_current(&expected, &changed),
                Err(VerbalixError::StaleSelection)
            ));
        }
    }

    #[test]
    fn indeterminate_write_reconciles_on_the_same_retained_handle() {
        let source = include_str!("macos_replace.rs");
        let write = &source[source
            .find("pub(super) fn write_on_element")
            .expect("write boundary")
            ..source.find("fn validate_expected").expect("validation")];
        let indeterminate = write
            .find("AxWriteResult::Indeterminate")
            .expect("typed indeterminate branch");
        let reconciliation = write
            .find("macos_selection_revalidation::read(element, expected.extraction_strategy)")
            .expect("same-handle reconciliation");

        assert!(reconciliation > indeterminate);
        assert!(!write.contains(concat!("focused_element_for_", "pid")));
    }
}
