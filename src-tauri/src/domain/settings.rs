use crate::domain::VerbalixError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LengthPreference {
    Concise,
    Balanced,
    Detailed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TonePreference {
    Neutral,
    Friendly,
    Assertive,
    Technical,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub formality: u8,
    pub length: LengthPreference,
    pub tone: TonePreference,
    pub confirm_before_replace: bool,
    pub history_enabled: bool,
    pub automatic_toolbar: bool,
    pub shortcut: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            formality: 3,
            length: LengthPreference::Balanced,
            tone: TonePreference::Technical,
            confirm_before_replace: false,
            history_enabled: false,
            automatic_toolbar: true,
            shortcut: "Option+Shift+Space".to_owned(),
        }
    }
}

impl AppSettings {
    pub fn validate(self) -> Result<Self, VerbalixError> {
        if !(1..=5).contains(&self.formality) || self.shortcut.trim().is_empty() {
            return Err(VerbalixError::LocalFailure);
        }
        Ok(self)
    }
}

pub trait SettingsRepository: Send + Sync {
    fn load(&self) -> Result<AppSettings, VerbalixError>;
    fn save(&self, settings: &AppSettings) -> Result<(), VerbalixError>;
}
