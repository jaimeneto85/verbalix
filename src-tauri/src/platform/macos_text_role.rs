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

    fn capture_trace(role: &str) -> (Result<(), VerbalixError>, Vec<&'static str>) {
        let mut trace = vec!["role"];
        let result = validate(role).map(|_| {
            trace.extend([
                "identifier",
                "selected_text",
                "selected_range",
                "value",
                "marker",
                "bounds",
            ]);
        });
        (result, trace)
    }

    #[test]
    fn blocked_roles_never_reach_the_content_reader() {
        for role in ["AXSecureTextField", "AXButton", "AXGroup", "AXImage", ""] {
            let (result, trace) = capture_trace(role);
            assert!(result.is_err());
            assert_eq!(trace, ["role"]);
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
            let (result, trace) = capture_trace(role);
            result.unwrap();
            assert_eq!(
                trace,
                [
                    "role",
                    "identifier",
                    "selected_text",
                    "selected_range",
                    "value",
                    "marker",
                    "bounds",
                ]
            );
        }
    }
}
