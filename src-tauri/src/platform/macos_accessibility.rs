use super::{
    macos_ax::{self, AXUIElementRef, OwnedAxElement},
    macos_focus::{AxCategory, AxStage, ExtractionOrigin},
    macos_selection,
};
use crate::{
    application::SelectionPort,
    domain::{SelectionSnapshot, VerbalixError},
};
use std::sync::Arc;

pub(super) const AX_SUCCESS: i32 = macos_ax::AX_SUCCESS;

pub struct MacAccessibility;

impl MacAccessibility {
    pub fn new() -> Self {
        Self
    }

    pub fn start_observer(&self, callback: Arc<dyn Fn() + Send + Sync>) {
        super::macos_observer::start(callback);
    }

    pub(super) fn focused_element() -> Result<OwnedAxElement, VerbalixError> {
        macos_ax::focused_element().map_err(|_| VerbalixError::SelectionUnavailable)
    }

    pub(super) fn string_attribute(
        element: AXUIElementRef,
        name: &str,
    ) -> Result<String, VerbalixError> {
        macos_ax::string_attribute(
            element,
            name,
            AxStage::SelectedText,
            ExtractionOrigin::SelectedText,
        )
        .map_err(|_| VerbalixError::SelectionUnavailable)
    }

    pub(super) fn writable(element: AXUIElementRef) -> bool {
        macos_ax::writable(element)
    }
}

impl SelectionPort for MacAccessibility {
    fn permission_granted(&self, prompt: bool) -> bool {
        macos_ax::trusted(prompt)
    }

    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError> {
        if !self.permission_granted(false) {
            crate::diagnostics::ax_resolution(
                AxStage::Trust,
                ExtractionOrigin::SelectedText,
                AxCategory::ApiDisabled,
            );
            return Err(VerbalixError::PermissionDenied);
        }
        crate::diagnostics::ax_resolution(
            AxStage::Trust,
            ExtractionOrigin::SelectedText,
            AxCategory::Success,
        );
        let element = Self::focused_element()?;
        macos_selection::capture(&element)
    }

    fn replace(&self, expected: &SelectionSnapshot, text: &str) -> Result<(), VerbalixError> {
        if !expected.writable || expected.element_identity.is_none() {
            return Err(VerbalixError::StaleSelection);
        }
        let element = Self::focused_element()?;
        let current = macos_selection::capture(&element)?;
        if !current.same_target(expected) || !current.writable {
            return Err(VerbalixError::StaleSelection);
        }
        macos_ax::set_selected_text(element.as_ref(), text)
            .then_some(())
            .ok_or(VerbalixError::LocalFailure)
    }

    fn restore(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
    ) -> Result<(), VerbalixError> {
        super::macos_restore::restore(expected, transformed_text)
    }
}
