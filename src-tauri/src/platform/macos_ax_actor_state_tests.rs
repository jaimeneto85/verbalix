use super::*;
use crate::{
    application::{MutationReceipt, MutationStatus},
    domain::{Rect, SelectionElementIdentity, TextRange},
    platform::{
        macos_ax_target::tests::InstrumentedAxTarget, macos_mutation_ledger::ReplaceTerminalOutcome,
    },
};

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
    .with_element_identity(SelectionElementIdentity {
        role: "AXTextField".to_owned(),
        subrole: None,
        frame: Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
    })
    .with_native_element_identifier(Some("editor".to_owned()))
}

fn state_with_target() -> (ActorState, SelectionSnapshot, Rc<InstrumentedAxTarget>) {
    let epoch = CausalEpoch::default();
    let mut state = ActorState::new(epoch.clone(), SelfNotificationSignal::default());
    let selected = snapshot();
    let instrumented = InstrumentedAxTarget::secure_after_prepare();
    let target = CapturedTarget {
        target: instrumented.clone(),
        epoch: epoch.current(),
        token: AxElementToken::new(selected.pid, "editor"),
    };
    let now = state.now();
    state.targets.insert(selected.id, target, now);
    (state, selected, instrumented)
}

fn receipt(selected: &SelectionSnapshot) -> MutationReceipt {
    MutationReceipt {
        id: Uuid::new_v4(),
        snapshot_id: selected.id,
        request_id: Uuid::nil(),
    }
}

#[test]
fn actor_replace_rejects_secure_after_prepare_on_the_exact_retained_target() {
    let (mut state, selected, instrumented) = state_with_target();
    let mutation = receipt(&selected);
    let result = state.replace(mutation.clone(), selected, "after".to_owned(), None);
    let projection = state
        .mutations
        .projection(mutation.id, state.now())
        .unwrap();
    assert!(result.is_err());
    assert!(projection.status == MutationStatus::Rejected);
    assert_eq!(instrumented.prepares(), 1);
    assert_eq!(instrumented.setters(), 0);
    assert!(!state.self_notifications.has_pending());
}

#[test]
fn actor_restore_rejects_secure_after_prepare_on_the_exact_retained_target() {
    let (mut state, selected, instrumented) = state_with_target();
    let mutation = receipt(&selected);
    let now = state.now();
    let target = state.targets.get(selected.id, now).unwrap().clone();
    state
        .mutations
        .prepare(
            mutation.clone(),
            selected.clone(),
            "after".to_owned(),
            target,
            now,
        )
        .unwrap();
    state
        .mutations
        .finish_replace(mutation.id, ReplaceTerminalOutcome::Confirmed, state.now())
        .unwrap();
    let result = state.restore(mutation.id, selected, "after".to_owned(), None);
    let projection = state
        .mutations
        .projection(mutation.id, state.now())
        .unwrap();
    assert!(result.is_err());
    assert!(projection.status == MutationStatus::RestoreRejected);
    assert_eq!(instrumented.prepares(), 1);
    assert_eq!(instrumented.setters(), 0);
    assert!(!state.self_notifications.has_pending());
}
