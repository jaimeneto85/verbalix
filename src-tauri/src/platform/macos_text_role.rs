use crate::domain::VerbalixError;

#[derive(Clone, Copy)]
pub(super) struct TextRoleCapability(());

pub(super) struct ValidatedTextRole {
    pub(super) role: String,
    pub(super) subrole: Option<String>,
    pub(super) capability: TextRoleCapability,
}

pub(super) fn validate(
    role: String,
    subrole: Option<String>,
) -> Result<ValidatedTextRole, VerbalixError> {
    if role == "AXSecureTextField" || subrole.as_deref() == Some("AXSecureTextField") {
        return Err(VerbalixError::ProtectedField);
    }
    let capability = matches!(
        role.as_str(),
        "AXTextArea" | "AXTextField" | "AXStaticText" | "AXWebArea" | "AXComboBox"
    )
    .then_some(TextRoleCapability(()))
    .ok_or(VerbalixError::SelectionUnavailable)?;
    Ok(ValidatedTextRole {
        role,
        subrole,
        capability,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capture_trace(
        role: &str,
        subrole: Option<&str>,
    ) -> (Result<(), VerbalixError>, Vec<&'static str>) {
        let mut trace = vec!["role", "subrole"];
        let result = validate(role.to_owned(), subrole.map(str::to_owned)).map(|_| {
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
        for (role, subrole) in [
            ("AXSecureTextField", None),
            ("AXTextField", Some("AXSecureTextField")),
            ("AXButton", None),
            ("AXGroup", None),
            ("AXImage", None),
            ("", None),
        ] {
            let (result, trace) = capture_trace(role, subrole);
            assert!(result.is_err());
            assert_eq!(trace, ["role", "subrole"]);
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
            let (result, trace) = capture_trace(role, None);
            result.unwrap();
            assert_eq!(
                trace,
                [
                    "role",
                    "subrole",
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
