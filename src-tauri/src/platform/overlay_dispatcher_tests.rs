use super::*;

#[test]
fn asynchronous_execution_failure_is_reported_once_without_waiting() {
    let failure = ExecutionFailure::default();

    failure.record();

    assert!(matches!(failure.take(), Err(VerbalixError::LocalFailure)));
    assert!(failure.take().is_ok());
}
