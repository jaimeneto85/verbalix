use crate::domain::VerbalixError;

#[derive(Clone, Copy)]
pub(super) struct TextRoleCapability(());

pub(super) fn validate(role: &str) -> Result<TextRoleCapability, VerbalixError> {
    if role == "AXSecureTextField" {
        return Err(VerbalixError::ProtectedField);
    }
    matches!(
        role,
        "AXTextArea" | "AXTextField" | "AXStaticText" | "AXWebArea" | "AXComboBox"
    )
    .then_some(TextRoleCapability(()))
    .ok_or(VerbalixError::SelectionUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_roles_never_reach_the_content_reader() {
        for role in ["AXSecureTextField", "AXButton", "AXGroup", "AXImage", ""] {
            let mut reads = 0;
            let result = validate(role).map(|_| reads += 1);
            assert!(result.is_err());
            assert_eq!(reads, 0);
        }
    }

    #[test]
    fn supported_text_roles_receive_one_capability() {
        for role in [
            "AXTextArea",
            "AXTextField",
            "AXStaticText",
            "AXWebArea",
            "AXComboBox",
        ] {
            let mut reads = 0;
            validate(role).map(|_| reads += 1).unwrap();
            assert_eq!(reads, 1);
        }
    }
}
