mod endpointing;
mod error;
mod live_interpretation;
mod selection;
mod settings;
mod transform;
mod voice;

pub use endpointing::{EndpointEvent, Endpointer, EndpointerConfig};
pub use error::VerbalixError;
pub use live_interpretation::{
    InterpretOutcome, LanguageTag, LiveSession, LiveSessionId, LiveState, SegmentId, SegmentResult,
    StageDurations,
};
pub use selection::{
    GeometrySource, Rect, SelectionElementIdentity, SelectionEvent, SelectionExtractionStrategy,
    SelectionSnapshot, SelectionState, TextRange,
};
pub use settings::{AppSettings, LengthPreference, SettingsRepository, TonePreference};
pub use transform::{
    AiProvider, TransformOperation, TransformPreferences, TransformRequest, TransformResult,
};
pub use voice::{EnrollmentSample, MicrophonePermission, VoiceProfileStatus, VoiceProfileView};
