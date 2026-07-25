use super::{
    causal_epoch::CausalEpoch,
    macos_ax::{self, OwnedAxElement},
    macos_ax_actor::AxActor,
    macos_ax_actor_observation::ObservedSelectionChange,
    macos_element_token::AxElementToken,
    macos_focus::{AxCategory, AxStage, ExtractionOrigin},
    macos_observer::{AccessibilityEvent, AccessibilityEventKind},
};
use crate::{
    application::{MutationProjection, MutationReceipt, PublicationGuard, SelectionPort},
    domain::{SelectionSnapshot, VerbalixError},
};
use std::sync::Arc;

pub struct MacAccessibility {
    actor: Arc<AxActor>,
}

impl MacAccessibility {
    pub fn new() -> Self {
        Self {
            actor: Arc::new(AxActor::new()),
        }
    }

    pub fn start_observer(&self, callback: Arc<dyn Fn() + Send + Sync>) {
        let epoch = self.actor.causal_epoch();
        let actor = self.actor.clone();
        super::macos_observer::start(Arc::new(move |event| {
            if route_observer_event(event, &epoch, |target, generation| {
                actor.observe_selection_change(target, generation)
            }) {
                callback();
            }
        }));
    }

    pub fn signal_causal_change(&self) {
        self.actor.signal_causal_change();
    }

    pub(super) fn focused_element() -> Result<OwnedAxElement, VerbalixError> {
        macos_ax::focused_element().map_err(|_| VerbalixError::SelectionUnavailable)
    }
}

pub(super) fn route_observer_event(
    event: AccessibilityEvent,
    epoch: &CausalEpoch,
    classify: impl FnOnce(AxElementToken, u64) -> Result<ObservedSelectionChange, VerbalixError>,
) -> bool {
    match event.kind {
        AccessibilityEventKind::FocusChanged | AccessibilityEventKind::ElementDestroyed => {
            epoch.bump();
            true
        }
        AccessibilityEventKind::SelectedTextChanged => {
            let generation = epoch.current();
            let own_change = event
                .target
                .and_then(|target| classify(target, generation).ok())
                == Some(ObservedSelectionChange::SelfGenerated);
            if own_change {
                false
            } else {
                epoch.bump();
                true
            }
        }
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
        self.actor
            .replace(expected, text, None, uuid::Uuid::new_v4())
            .map(|_| ())
    }

    fn replace_guarded(
        &self,
        expected: &SelectionSnapshot,
        text: &str,
        lease: &PublicationGuard,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.actor
            .replace(expected, text, Some(lease.clone()), uuid::Uuid::new_v4())
    }

    fn replace_guarded_with_id(
        &self,
        expected: &SelectionSnapshot,
        text: &str,
        lease: &PublicationGuard,
        mutation_id: uuid::Uuid,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.actor
            .replace(expected, text, Some(lease.clone()), mutation_id)
    }

    fn restore(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
    ) -> Result<(), VerbalixError> {
        self.actor
            .restore(uuid::Uuid::new_v4(), expected, transformed_text, None)
            .map(|_| ())
    }

    fn restore_guarded(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
        lease: &PublicationGuard,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.actor.restore(
            uuid::Uuid::new_v4(),
            expected,
            transformed_text,
            Some(lease.clone()),
        )
    }

    fn restore_guarded_with_id(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
        lease: &PublicationGuard,
        mutation_id: uuid::Uuid,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.actor
            .restore(mutation_id, expected, transformed_text, Some(lease.clone()))
    }

    fn discard_snapshot(&self, snapshot_id: uuid::Uuid) {
        self.actor.discard(snapshot_id);
    }

    fn reconcile_mutation(
        &self,
        mutation_id: uuid::Uuid,
    ) -> Result<Option<MutationProjection>, VerbalixError> {
        self.actor.reconcile(mutation_id)
    }
}

#[cfg(test)]
#[path = "macos_accessibility_observer_tests.rs"]
mod observer_tests;

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
