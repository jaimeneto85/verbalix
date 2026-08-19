mod error;
mod selection;
mod settings;
mod transform;
mod voice;

pub use error::VerbalixError;
pub use selection::{
    GeometrySource, Rect, SelectionElementIdentity, SelectionEvent, SelectionExtractionStrategy,
    SelectionSnapshot, SelectionState, TextRange,
};
pub use settings::{AppSettings, LengthPreference, SettingsRepository, TonePreference};
pub use transform::{
    AiProvider, TransformOperation, TransformPreferences, TransformRequest, TransformResult,
};
pub use voice::{EnrollmentSample, MicrophonePermission, VoiceProfileStatus, VoiceProfileView};
