use crate::{application::PublicationGuard, domain::VerbalixError};

pub(super) fn execute_if_publishable(
    guard: Option<&PublicationGuard>,
    prepare: impl FnOnce() -> Result<(), VerbalixError>,
    publish: impl FnOnce() -> Result<(), VerbalixError>,
    cleanup: impl FnOnce() -> Result<(), VerbalixError>,
) -> Result<bool, VerbalixError> {
    if guard.is_some_and(|guard| !guard.may_publish()) {
        return Ok(false);
    }
    let mut cleanup = Some(cleanup);
    if let Err(error) = prepare() {
        cleanup.take().expect("cleanup is available")()?;
        return Err(error);
    }
    if guard.is_some_and(|guard| !guard.try_claim_publication()) {
        cleanup.take().expect("cleanup is available")()?;
        return Ok(false);
    }
    if let Err(error) = publish() {
        cleanup.take().expect("cleanup is available")()?;
        return Err(error);
    }
    Ok(true)
}

#[cfg(test)]
#[path = "overlay_publication_tests.rs"]
mod tests;
