use super::{
    macos_ax::{self, AXUIElementRef, AxWriteResult},
    macos_selection, macos_text_role,
    macos_write_authorization::AxWriteAuthorization,
};
use crate::domain::{SelectionElementIdentity, SelectionSnapshot, VerbalixError};

pub(super) fn set_selected_text(
    expected: &SelectionSnapshot,
    text: &str,
    element: AXUIElementRef,
    authorization: &AxWriteAuthorization,
) -> Result<AxWriteResult, VerbalixError> {
    let write = macos_ax::prepare_selected_text_write(element, text);
    let expected_identity = expected
        .element_identity
        .as_ref()
        .ok_or(VerbalixError::StaleSelection)?;
    let current_role = macos_selection::text_role(element).map_err(|error| match error {
        VerbalixError::ProtectedField => error,
        _ => VerbalixError::StaleSelection,
    })?;
    set_prepared_after_role_validation(
        expected_identity,
        current_role,
        authorization,
        write,
        macos_ax::set_prepared_selected_text,
    )
}

fn set_prepared_after_role_validation<T>(
    expected: &SelectionElementIdentity,
    current: macos_text_role::ValidatedTextRole,
    authorization: &AxWriteAuthorization,
    write: T,
    setter: impl FnOnce(T) -> AxWriteResult,
) -> Result<AxWriteResult, VerbalixError> {
    if current.role != expected.role
        || current.subrole != expected.subrole
        || !authorization.begin_setter()
    {
        return Err(VerbalixError::StaleSelection);
    }
    let result = setter(write);
    authorization.commit();
    Ok(result)
}

#[cfg(test)]
pub(super) fn set_after_role_validation(
    expected: &SelectionElementIdentity,
    current: macos_text_role::ValidatedTextRole,
    authorization: &AxWriteAuthorization,
    setter: impl FnOnce() -> AxWriteResult,
) -> Result<AxWriteResult, VerbalixError> {
    if current.role != expected.role
        || current.subrole != expected.subrole
        || !authorization.begin_setter()
    {
        return Err(VerbalixError::StaleSelection);
    }
    let result = setter();
    authorization.commit();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{MutationReceipt, MutationStatus},
        domain::{Rect, TextRange},
        platform::macos_mutation_ledger::{
            MutationLedger, ReplaceTerminalOutcome, RestoreTerminalOutcome,
        },
    };
    use std::cell::Cell;
    use uuid::Uuid;

    fn identity() -> SelectionElementIdentity {
        SelectionElementIdentity {
            role: "AXTextField".to_owned(),
            subrole: None,
            frame: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
        }
    }

    fn snapshot() -> SelectionSnapshot {
        SelectionSnapshot::new(
            42,
            "pid:42".to_owned(),
            "before".to_owned(),
            TextRange {
                location: 0,
                length: 6,
            },
            Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            true,
        )
        .with_element_identity(identity())
    }
    fn authorization() -> AxWriteAuthorization {
        AxWriteAuthorization::new(
            crate::platform::causal_epoch::CausalEpoch::default(),
            0,
            crate::platform::macos_self_notification_phase::SelfNotificationPhase::armed(),
        )
    }
    #[test]
    fn secure_or_changed_role_rejects_before_the_setter() {
        for current in [
            macos_text_role::validate(
                "AXTextField".to_owned(),
                Some("AXSecureTextField".to_owned()),
            ),
            macos_text_role::validate("AXTextArea".to_owned(), None),
        ] {
            let setters = Cell::new(0);
            let result = current.and_then(|role| {
                set_after_role_validation(&identity(), role, &authorization(), || {
                    setters.set(setters.get() + 1);
                    AxWriteResult::Confirmed
                })
            });
            assert!(result.is_err());
            assert_eq!(setters.get(), 0);
        }
    }
    #[test]
    fn matching_role_invokes_exactly_one_setter() {
        let setters = Cell::new(0);
        let current = macos_text_role::validate("AXTextField".to_owned(), None).unwrap();
        let result = set_after_role_validation(&identity(), current, &authorization(), || {
            setters.set(setters.get() + 1);
            AxWriteResult::Confirmed
        });
        assert!(matches!(result, Ok(AxWriteResult::Confirmed)));
        assert_eq!(setters.get(), 1);
    }

    #[test]
    fn prepared_payload_crosses_final_authorization_directly_into_one_setter() {
        let trace = std::cell::RefCell::new(Vec::new());
        let write = macos_ax::prepare_selected_text_write(std::ptr::null_mut(), "after");
        trace.borrow_mut().push("prepared");
        let phase = crate::platform::macos_self_notification_phase::SelfNotificationPhase::armed();
        let authorization = AxWriteAuthorization::new(
            crate::platform::causal_epoch::CausalEpoch::default(),
            0,
            phase.clone(),
        );
        let current = macos_text_role::validate("AXTextField".to_owned(), None).unwrap();
        let result = set_prepared_after_role_validation(
            &identity(),
            current,
            &authorization,
            write,
            |write| {
                assert!(phase.claim_observation());
                trace.borrow_mut().push("setter");
                drop(write);
                AxWriteResult::Confirmed
            },
        );
        assert!(matches!(result, Ok(AxWriteResult::Confirmed)));
        assert_eq!(*trace.borrow(), ["prepared", "setter"]);
    }
    #[test]
    fn rejected_final_authorization_never_reaches_the_setter() {
        let setters = Cell::new(0);
        let write = macos_ax::prepare_selected_text_write(std::ptr::null_mut(), "after");
        let epoch = crate::platform::causal_epoch::CausalEpoch::default();
        let authorization = AxWriteAuthorization::new(
            epoch.clone(),
            epoch.current(),
            crate::platform::macos_self_notification_phase::SelfNotificationPhase::armed(),
        );
        epoch.bump();
        let current = macos_text_role::validate("AXTextField".to_owned(), None).unwrap();
        let result = set_prepared_after_role_validation(
            &identity(),
            current,
            &authorization,
            write,
            |write| {
                setters.set(setters.get() + 1);
                drop(write);
                AxWriteResult::Confirmed
            },
        );
        assert!(matches!(result, Err(VerbalixError::StaleSelection)));
        assert_eq!(setters.get(), 0);
    }
    #[test]
    fn stale_epoch_cancels_authorizing_before_it_can_become_a_self_candidate() {
        let epoch = crate::platform::causal_epoch::CausalEpoch::default();
        let phase = crate::platform::macos_self_notification_phase::SelfNotificationPhase::armed();
        let authorization =
            AxWriteAuthorization::new(epoch.clone(), epoch.current(), phase.clone());

        assert!(!authorization.begin_setter_after_authorizing(|| {
            epoch.bump();
        }));
        assert!(!phase.claim_observation());
    }
    #[test]
    fn causal_cancel_after_epoch_check_makes_setter_promotion_lose() {
        let epoch = crate::platform::causal_epoch::CausalEpoch::default();
        let phase = crate::platform::macos_self_notification_phase::SelfNotificationPhase::armed();
        let authorization =
            AxWriteAuthorization::new(epoch.clone(), epoch.current(), phase.clone());

        assert!(!authorization.begin_setter_after_epoch_valid(|| {
            assert!(phase.cancel_before_setter());
            epoch.bump();
        }));
        assert!(!phase.claim_observation());
    }

    #[test]
    fn secure_after_prepare_rejects_replace_and_restore_without_a_setter() {
        let selected = snapshot();
        let receipt = MutationReceipt {
            id: Uuid::new_v4(),
            snapshot_id: selected.id,
            request_id: Uuid::new_v4(),
        };
        let setters = Cell::new(0);
        let mut ledger = MutationLedger::new(1);
        ledger
            .prepare(receipt.clone(), selected.clone(), "after".to_owned(), (), 0)
            .unwrap();
        let secured = macos_text_role::validate(
            "AXTextField".to_owned(),
            Some("AXSecureTextField".to_owned()),
        )
        .and_then(|role| {
            set_after_role_validation(&identity(), role, &authorization(), || {
                setters.set(setters.get() + 1);
                AxWriteResult::Confirmed
            })
        });
        assert!(secured.is_err());
        let rejected = ledger
            .finish_replace(receipt.id, ReplaceTerminalOutcome::Rejected, 1)
            .unwrap();
        assert!(rejected.status == MutationStatus::Rejected);
        assert_eq!(setters.get(), 0);

        let restore_receipt = MutationReceipt {
            id: Uuid::new_v4(),
            snapshot_id: selected.id,
            request_id: Uuid::new_v4(),
        };
        let mut restore_ledger = MutationLedger::new(1);
        restore_ledger
            .prepare(restore_receipt.clone(), selected, "after".to_owned(), (), 0)
            .unwrap();
        restore_ledger
            .finish_replace(restore_receipt.id, ReplaceTerminalOutcome::Confirmed, 1)
            .unwrap();
        restore_ledger.begin_restore(restore_receipt.id, 2).unwrap();
        let secured_restore = macos_text_role::validate(
            "AXTextField".to_owned(),
            Some("AXSecureTextField".to_owned()),
        )
        .and_then(|role| {
            set_after_role_validation(&identity(), role, &authorization(), || {
                setters.set(setters.get() + 1);
                AxWriteResult::Confirmed
            })
        });
        assert!(secured_restore.is_err());
        restore_ledger
            .finish_restore(restore_receipt.id, RestoreTerminalOutcome::Rejected, 3)
            .unwrap();
        assert!(
            restore_ledger
                .projection(restore_receipt.id, 3)
                .unwrap()
                .status
                == MutationStatus::RestoreRejected
        );
        assert_eq!(setters.get(), 0);
    }
}
