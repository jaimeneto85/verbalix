use crate::domain::VerbalixError;

pub(crate) fn record(event: &str, error: Option<&VerbalixError>) {
    let metadata = error
        .map(|error| format!("error={}", super::error_code(error)))
        .unwrap_or_else(|| "error=none".to_owned());
    super::emit("history", event, &metadata);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_metadata_uses_only_a_sanitized_error_code() {
        let error = VerbalixError::ProviderRejected;
        let metadata = format!("error={}", super::super::error_code(&error));

        assert_eq!(metadata, "error=provider_rejected");
    }
}
