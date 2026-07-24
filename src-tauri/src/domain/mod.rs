mod error;
mod selection;
mod settings;
mod transform;

pub use error::VerbalixError;
pub use selection::{
    GeometrySource, Rect, SelectionElementIdentity, SelectionEvent, SelectionSnapshot,
    SelectionState, TextRange,
};
pub use settings::{AppSettings, LengthPreference, SettingsRepository, TonePreference};
pub use transform::{
    AiProvider, TransformOperation, TransformPreferences, TransformRequest, TransformResult,
};
