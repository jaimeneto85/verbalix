use super::{
    macos_ax::{self, AXUIElementRef},
    macos_focus::{AxStage, ExtractionOrigin},
    macos_selection,
};
use crate::domain::VerbalixError;

#[derive(Clone, Eq, PartialEq)]
pub(super) struct AxElementToken {
    pub(super) pid: i32,
    identifier: String,
}

impl AxElementToken {
    pub(super) fn new(pid: i32, identifier: &str) -> Option<Self> {
        (pid > 0 && !identifier.trim().is_empty()).then(|| Self {
            pid,
            identifier: identifier.to_owned(),
        })
    }
}

impl std::fmt::Debug for AxElementToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AxElementToken")
            .field("pid", &self.pid)
            .field("identifier", &"<redacted>")
            .finish()
    }
}

pub(super) fn read(element: AXUIElementRef) -> Result<Option<AxElementToken>, VerbalixError> {
    read_after_role(macos_selection::text_role(element), || {
        let pid = macos_ax::pid(element).map_err(|_| VerbalixError::StaleSelection)?;
        let identifier = macos_ax::optional_string_attribute(
            element,
            "AXIdentifier",
            AxStage::Identifier,
            ExtractionOrigin::SelectedText,
        )
        .map_err(|_| VerbalixError::StaleSelection)?;
        Ok(identifier.and_then(|identifier| AxElementToken::new(pid, &identifier)))
    })
}

fn read_after_role<T>(
    role: Result<super::macos_text_role::ValidatedTextRole, VerbalixError>,
    reader: impl FnOnce() -> Result<T, VerbalixError>,
) -> Result<T, VerbalixError> {
    let _ = role?.capability;
    reader()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn exact_identifier_is_part_of_equality_but_never_debug_output() {
        let first = AxElementToken::new(42, "editor-a").unwrap();
        let same = AxElementToken::new(42, "editor-a").unwrap();
        let other = AxElementToken::new(42, "editor-b").unwrap();

        assert_eq!(first, same);
        assert_ne!(first, other);
        assert!(!format!("{first:?}").contains("editor-a"));
        assert!(AxElementToken::new(42, "").is_none());
    }

    #[test]
    fn secure_role_blocks_identifier_token_reader() {
        let reads = Cell::new(0);
        let secure = super::super::macos_text_role::validate(
            "AXTextField".to_owned(),
            Some("AXSecureTextField".to_owned()),
        );
        let result = read_after_role(secure, || {
            reads.set(reads.get() + 1);
            Ok(AxElementToken::new(42, "secret-editor"))
        });

        assert!(matches!(result, Err(VerbalixError::ProtectedField)));
        assert_eq!(reads.get(), 0);
    }
}
