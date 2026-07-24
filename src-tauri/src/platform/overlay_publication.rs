use crate::{application::PublicationGuard, domain::VerbalixError};

pub(super) fn execute_if_publishable(
    guard: Option<&PublicationGuard>,
    execute: impl FnOnce() -> Result<(), VerbalixError>,
) -> Result<bool, VerbalixError> {
    if guard.is_some_and(|guard| !guard.may_publish()) {
        return Ok(false);
    }
    execute()?;
    Ok(true)
}
