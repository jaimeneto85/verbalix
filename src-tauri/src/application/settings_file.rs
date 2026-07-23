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
