use super::{
    macos_ax::{self, OwnedAxElement},
    macos_ax_actor::AxActor,
    macos_focus::{AxCategory, AxStage, ExtractionOrigin},
};
use crate::{
    application::{MutationReceipt, PublicationGuard, SelectionPort},
    domain::{SelectionSnapshot, VerbalixError},
};
use std::sync::Arc;

pub struct MacAccessibility {
    actor: AxActor,
}

impl MacAccessibility {
    pub fn new() -> Self {
        Self {
            actor: AxActor::new(),
        }
    }

    pub fn start_observer(&self, callback: Arc<dyn Fn() + Send + Sync>) {
        super::macos_observer::start(callback);
    }

    pub(super) fn focused_element() -> Result<OwnedAxElement, VerbalixError> {
        macos_ax::focused_element().map_err(|_| VerbalixError::SelectionUnavailable)
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
        self.actor.capture()
    }

    fn replace(&self, expected: &SelectionSnapshot, text: &str) -> Result<(), VerbalixError> {
        self.actor.replace(expected, text, None).map(|_| ())
    }

    fn replace_guarded(
        &self,
        expected: &SelectionSnapshot,
        text: &str,
        lease: &PublicationGuard,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.actor.replace(expected, text, Some(lease.clone()))
    }

    fn restore(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
    ) -> Result<(), VerbalixError> {
        self.actor
            .restore(expected, transformed_text, None)
            .map(|_| ())
    }

    fn restore_guarded(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
        lease: &PublicationGuard,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.actor
            .restore(expected, transformed_text, Some(lease.clone()))
    }

    fn discard_snapshot(&self, snapshot_id: uuid::Uuid) {
        self.actor.discard(snapshot_id);
    }
}

#[cfg(test)]
fn replacement_eligible(expected: &SelectionSnapshot) -> bool {
    expected.writable
        && expected
            .element_identity
            .as_ref()
            .and_then(|identity| identity.strong_identifier())
            .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Rect, SelectionElementIdentity, TextRange};

    fn snapshot(writable: bool, identity: bool) -> SelectionSnapshot {
        let snapshot = SelectionSnapshot::new(
            42,
            "pid:42".to_owned(),
            "selected".to_owned(),
            TextRange {
                location: 1,
                length: 8,
            },
            Rect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            },
            writable,
        );
        if identity {
            snapshot.with_element_identity(SelectionElementIdentity {
                role: "AXTextArea".to_owned(),
                subrole: None,
                identifier: Some("editor".to_owned()),
                frame: Rect {
                    x: 1.0,
                    y: 2.0,
                    width: 3.0,
                    height: 4.0,
                },
            })
        } else {
            snapshot
        }
    }

    #[test]
    fn replacement_fails_before_ax_for_read_only_or_unidentified_snapshots() {
        let mut weak_identity = snapshot(true, true);
        weak_identity.element_identity.as_mut().unwrap().identifier = None;
        let mut empty_identity = snapshot(true, true);
        empty_identity.element_identity.as_mut().unwrap().identifier = Some(String::new());
        let mut whitespace_identity = snapshot(true, true);
        whitespace_identity
            .element_identity
            .as_mut()
            .unwrap()
            .identifier = Some("  ".to_owned());

        assert!(!replacement_eligible(&snapshot(false, true)));
        assert!(!replacement_eligible(&snapshot(true, false)));
        assert!(!replacement_eligible(&weak_identity));
        assert!(!replacement_eligible(&empty_identity));
        assert!(!replacement_eligible(&whitespace_identity));
        assert!(replacement_eligible(&snapshot(true, true)));
    }
}
