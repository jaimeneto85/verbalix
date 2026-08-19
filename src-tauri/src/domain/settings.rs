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

const ALLOWED_LANGUAGES: &[&str] = &["en", "pt", "es", "fr", "de", "it", "ja", "ko", "zh"];

fn default_target_language() -> String {
    "en".to_owned()
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
    #[serde(default)]
    pub voice_profile_id: Option<uuid::Uuid>,
    #[serde(default = "default_target_language")]
    pub target_language: String,
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
            voice_profile_id: None,
            target_language: "en".to_owned(),
        }
    }
}

impl AppSettings {
    pub fn validate(self) -> Result<Self, VerbalixError> {
        if !(1..=5).contains(&self.formality) || self.shortcut.trim().is_empty() {
            return Err(VerbalixError::LocalFailure);
        }
        if !ALLOWED_LANGUAGES.contains(&self.target_language.as_str()) {
            return Err(VerbalixError::LocalFailure);
        }
        Ok(self)
    }
}

pub trait SettingsRepository: Send + Sync {
    fn load(&self) -> Result<AppSettings, VerbalixError>;
    fn save(&self, settings: &AppSettings) -> Result<(), VerbalixError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_product_contract() {
        let settings = AppSettings::default();
        assert_eq!(settings.formality, 3);
        assert_eq!(settings.length, LengthPreference::Balanced);
        assert_eq!(settings.tone, TonePreference::Technical);
        assert!(!settings.confirm_before_replace);
    }

    #[test]
    fn rejects_formality_outside_supported_range() {
        let settings = AppSettings {
            formality: 6,
            ..AppSettings::default()
        };
        assert!(settings.validate().is_err());
    }
}
