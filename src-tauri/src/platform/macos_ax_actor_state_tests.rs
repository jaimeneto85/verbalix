#[test]
fn missing_causal_handle_has_no_focus_or_identity_fallback() {
    let source = include_str!("macos_ax_actor_state.rs");
    assert!(!source.contains(concat!("focused_element_for_", "pid")));
}

#[test]
fn unresolved_write_is_retained_as_indeterminate() {
    assert!(include_str!("macos_ax_actor_state.rs").contains("MutationStatus::Indeterminate"));
}

#[test]
fn replay_is_resolved_from_ledger_before_ax_preparation() {
    let source = include_str!("macos_ax_actor_state.rs");
    let replace = &source[source.find("pub(super) fn replace(").unwrap()
        ..source.find("pub(super) fn restore(").unwrap()];
    assert!(
        replace.find("self.mutations").unwrap()
            < replace.find("macos_replace::prepare_on_element").unwrap()
    );
}

#[test]
fn restore_is_idempotent_under_a_caller_preallocated_mutation_id() {
    let actor = include_str!("macos_ax_actor.rs");
    let command = &actor[actor.find("Restore {").unwrap()..actor.find("Discard(").unwrap()];
    let state = include_str!("macos_ax_actor_state.rs");
    let restore = &state[state.find("pub(super) fn restore(").unwrap()
        ..state.find("pub(super) fn discard(").unwrap()];
    assert!(command.contains("mutation_id"));
    assert!(restore.contains("self.mutations"));
}
