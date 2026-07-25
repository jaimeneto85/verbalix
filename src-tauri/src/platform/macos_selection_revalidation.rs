use super::{
    macos_ax::{self, AXUIElementRef},
    macos_classic_range::{self, CFRange},
    macos_focus::{AxStage, ExtractionOrigin},
    macos_selection, macos_text_role, macos_value_range,
};
use crate::domain::{SelectionExtractionStrategy, VerbalixError};

pub(super) struct CurrentSelection {
    pub(super) text: String,
    pub(super) range: CFRange,
    pub(super) strategy: SelectionExtractionStrategy,
}

pub(super) fn read(
    element: AXUIElementRef,
    strategy: SelectionExtractionStrategy,
) -> Result<CurrentSelection, VerbalixError> {
    let role = macos_selection::text_role(element).map_err(|error| match error {
        VerbalixError::ProtectedField => error,
        _ => VerbalixError::StaleSelection,
    })?;
    read_authorized(role, || match strategy {
        SelectionExtractionStrategy::SelectedText => selected_text(element, strategy),
        SelectionExtractionStrategy::StringForRange => string_for_range(element, strategy),
        SelectionExtractionStrategy::ValueRange => value_range(element, strategy),
        SelectionExtractionStrategy::TextMarker => Err(VerbalixError::StaleSelection),
    })
}

fn read_authorized<T>(
    role: macos_text_role::ValidatedTextRole,
    reader: impl FnOnce() -> Result<T, VerbalixError>,
) -> Result<T, VerbalixError> {
    let _ = role.capability;
    reader()
}

fn selected_text(
    element: AXUIElementRef,
    strategy: SelectionExtractionStrategy,
) -> Result<CurrentSelection, VerbalixError> {
    let text = macos_ax::string_attribute(
        element,
        "AXSelectedText",
        AxStage::SelectedText,
        ExtractionOrigin::SelectedText,
    )
    .map_err(|_| VerbalixError::StaleSelection)?;
    let range =
        macos_classic_range::selected_range(element).map_err(|_| VerbalixError::StaleSelection)?;
    Ok(CurrentSelection {
        text,
        range,
        strategy,
    })
}

fn string_for_range(
    element: AXUIElementRef,
    strategy: SelectionExtractionStrategy,
) -> Result<CurrentSelection, VerbalixError> {
    let range =
        macos_classic_range::selected_range(element).map_err(|_| VerbalixError::StaleSelection)?;
    let text = macos_classic_range::string_for_range(element, range)
        .map_err(|_| VerbalixError::StaleSelection)?;
    Ok(CurrentSelection {
        text,
        range,
        strategy,
    })
}

fn value_range(
    element: AXUIElementRef,
    strategy: SelectionExtractionStrategy,
) -> Result<CurrentSelection, VerbalixError> {
    let selection =
        macos_value_range::extract(element).map_err(|_| VerbalixError::StaleSelection)?;
    Ok(CurrentSelection {
        text: selection.text,
        range: selection.range,
        strategy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn secure_transition_rejects_before_the_content_reader() {
        let reads = Cell::new(0);
        let secure = macos_text_role::validate(
            "AXTextField".to_owned(),
            Some("AXSecureTextField".to_owned()),
        );

        let result = secure.and_then(|role| {
            read_authorized(role, || {
                reads.set(reads.get() + 1);
                Ok(())
            })
        });

        assert!(matches!(result, Err(VerbalixError::ProtectedField)));
        assert_eq!(reads.get(), 0);
    }

    #[test]
    fn authorized_text_role_reaches_the_reader_once() {
        let reads = Cell::new(0);
        let role = macos_text_role::validate("AXTextArea".to_owned(), None).unwrap();

        read_authorized(role, || {
            reads.set(reads.get() + 1);
            Ok(())
        })
        .unwrap();

        assert_eq!(reads.get(), 1);
    }
}
