use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VerbalixError {
    #[error("Accessibility permission is required")]
    PermissionDenied,
    #[error("No supported text selection is available")]
    SelectionUnavailable,
    #[error("The selected field is protected")]
    ProtectedField,
    #[error("The selection changed before the operation completed")]
    StaleSelection,
    #[error("Another transformation is already in progress")]
    OperationInProgress,
    #[error("Selected text exceeds the 12,000 character limit")]
    TextTooLong,
    #[error("Remote authentication is required")]
    Unauthenticated,
    #[error("The public transformation provider configuration is missing")]
    ProviderNotConfigured,
    #[error("The transformation provider timed out")]
    ProviderTimeout,
    #[error("The transformation provider rejected the request")]
    ProviderRejected,
    #[error("The transformation response was invalid")]
    InvalidResponse,
    #[cfg(not(target_os = "macos"))]
    #[error("The requested operation is not supported on this platform")]
    UnsupportedPlatform,
    #[error("A local operation failed")]
    LocalFailure,
}

impl Serialize for VerbalixError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
