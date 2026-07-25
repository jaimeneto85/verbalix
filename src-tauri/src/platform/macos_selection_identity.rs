use super::{
    macos_ax::{self, AXUIElementRef},
    macos_focus::{AxCategory, AxStage, ExtractionOrigin},
    macos_geometry, macos_text_role,
};
use crate::domain::{SelectionElementIdentity, VerbalixError};

#[derive(PartialEq)]
pub(super) struct CapturedElementIdentity {
    pub(super) metadata: SelectionElementIdentity,
    pub(super) native_identifier: Option<String>,
}

pub(super) fn element_identity(
    element: AXUIElementRef,
    text_role: &macos_text_role::ValidatedTextRole,
) -> Result<SelectionElementIdentity, VerbalixError> {
    let frame = macos_geometry::element_frame(element).ok_or(VerbalixError::StaleSelection)?;
    Ok(SelectionElementIdentity {
        role: text_role.role.clone(),
        subrole: text_role.subrole.clone(),
        frame,
    })
}

pub(super) fn native_identifier(element: AXUIElementRef) -> Result<Option<String>, VerbalixError> {
    let origin = ExtractionOrigin::SelectedText;
    let identifier =
        macos_ax::optional_string_attribute(element, "AXIdentifier", AxStage::Role, origin)
            .map_err(|_| VerbalixError::StaleSelection)?;
    crate::diagnostics::ax_resolution(
        AxStage::Identifier,
        origin,
        if identifier
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            AxCategory::Success
        } else {
            AxCategory::NoValue
        },
    );
    Ok(identifier.filter(|value| !value.trim().is_empty()))
}

pub(super) fn captured_element_identity(
    element: AXUIElementRef,
    text_role: &macos_text_role::ValidatedTextRole,
) -> Result<CapturedElementIdentity, VerbalixError> {
    Ok(CapturedElementIdentity {
        metadata: element_identity(element, text_role)?,
        native_identifier: native_identifier(element)?,
    })
}

pub(super) fn text_role(
    element: AXUIElementRef,
) -> Result<macos_text_role::ValidatedTextRole, VerbalixError> {
    let role = macos_ax::string_attribute(
        element,
        "AXRole",
        AxStage::Role,
        ExtractionOrigin::SelectedText,
    )
    .map_err(|_| VerbalixError::SelectionUnavailable)?;
    let subrole = macos_ax::optional_string_attribute(
        element,
        "AXSubrole",
        AxStage::Role,
        ExtractionOrigin::SelectedText,
    )
    .map_err(|_| VerbalixError::SelectionUnavailable)?;
    macos_text_role::validate(role, subrole)
}
