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
    #[allow(dead_code)]
    #[error("Permissão de microfone negada")]
    MicrophonePermissionDenied,
    #[error("Falha na captura de áudio")]
    AudioCaptureFailed,
    #[error("Falha no enrollment de voz")]
    EnrollmentFailed,
    #[error("A local operation failed")]
    LocalFailure,
    #[error("Sessão de interpretação ao vivo inativa")]
    LiveSessionInactive,
    #[error("Língua-alvo não suportada")]
    TargetLanguageUnsupported,
    #[error("Perfil de voz não encontrado")]
    VoiceProfileMissing,
    #[error("Falha na reprodução de áudio")]
    AudioPlaybackFailed,
    #[error("Falha na interpretação do segmento")]
    InterpretationFailed,
    #[error("Falha de STT no segmento")]
    SttFailed,
    #[error("Falha de tradução no segmento")]
    TranslationFailed,
    #[error("Falha de TTS no segmento")]
    TtsFailed,
}

impl Serialize for VerbalixError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
