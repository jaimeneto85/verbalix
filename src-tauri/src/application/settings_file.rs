use crate::domain::{AppSettings, SettingsRepository, VerbalixError};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct JsonSettingsRepository {
    path: PathBuf,
}

impl JsonSettingsRepository {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl SettingsRepository for JsonSettingsRepository {
    fn load(&self) -> Result<AppSettings, VerbalixError> {
        if !self.path.exists() {
            return Ok(AppSettings::default());
        }
        let bytes = fs::read(&self.path).map_err(|_| VerbalixError::LocalFailure)?;
        serde_json::from_slice::<AppSettings>(&bytes)
            .map_err(|_| VerbalixError::LocalFailure)?
            .validate()
    }

    fn save(&self, settings: &AppSettings) -> Result<(), VerbalixError> {
        settings.clone().validate()?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| VerbalixError::LocalFailure)?;
        }
        let bytes = serde_json::to_vec_pretty(settings).map_err(|_| VerbalixError::LocalFailure)?;
        let temporary = self.path.with_extension("tmp");
        fs::write(&temporary, bytes).map_err(|_| VerbalixError::LocalFailure)?;
        fs::rename(temporary, &self.path).map_err(|_| VerbalixError::LocalFailure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{LengthPreference, TonePreference};

    #[test]
    fn missing_file_returns_product_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let repository = JsonSettingsRepository::new(directory.path().join("settings.json"));

        assert_eq!(repository.load().unwrap(), AppSettings::default());
    }

    #[test]
    fn saves_and_loads_valid_settings_atomically() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested/settings.json");
        let repository = JsonSettingsRepository::new(&path);
        let settings = AppSettings {
            formality: 5,
            length: LengthPreference::Detailed,
            tone: TonePreference::Assertive,
            confirm_before_replace: true,
            history_enabled: true,
            automatic_toolbar: false,
            shortcut: "Option+Shift+Space".to_owned(),
        };

        repository.save(&settings).unwrap();

        assert_eq!(repository.load().unwrap(), settings);
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn corrupt_file_fails_closed_instead_of_resetting_preferences() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        std::fs::write(&path, b"{not-json").unwrap();
        let repository = JsonSettingsRepository::new(path);

        assert!(matches!(
            repository.load(),
            Err(VerbalixError::LocalFailure)
        ));
    }

    #[test]
    fn invalid_settings_are_not_persisted() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let repository = JsonSettingsRepository::new(&path);
        let settings = AppSettings {
            formality: 0,
            ..AppSettings::default()
        };

        assert!(repository.save(&settings).is_err());
        assert!(!path.exists());
    }
}
