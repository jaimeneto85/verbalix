use crate::domain::VerbalixError;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSyncMeta {
    pub updated_at_secs: Option<u64>,
    pub synced_at_secs: Option<u64>,
    pub sequence: u64,
}

pub struct PreferencesSyncStore {
    path: PathBuf,
}

impl PreferencesSyncStore {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn load(&self) -> Option<LocalSyncMeta> {
        let bytes = fs::read(&self.path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn record_change(&self, updated_at_secs: u64) -> Result<(), VerbalixError> {
        let mut meta = self.load().unwrap_or_default();
        meta.updated_at_secs = Some(updated_at_secs);
        meta.sequence += 1;
        self.write(&meta)
    }

    pub fn record_synced(&self, synced_at_secs: u64) -> Result<(), VerbalixError> {
        let mut meta = self.load().unwrap_or_default();
        meta.synced_at_secs = Some(synced_at_secs);
        self.write(&meta)
    }

    fn write(&self, meta: &LocalSyncMeta) -> Result<(), VerbalixError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| VerbalixError::LocalFailure)?;
        }
        let bytes = serde_json::to_vec_pretty(meta).map_err(|_| VerbalixError::LocalFailure)?;
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, bytes).map_err(|_| VerbalixError::LocalFailure)?;
        fs::rename(&tmp, &self.path).map_err(|_| VerbalixError::LocalFailure)
    }
}

pub fn epoch_secs_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn parse_iso8601_utc_secs(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let year: u64 = s.get(0..4)?.parse().ok()?;
    let month: u64 = s.get(5..7)?.parse().ok()?;
    let day: u64 = s.get(8..10)?.parse().ok()?;
    let hour: u64 = s.get(11..13)?.parse().ok()?;
    let minute: u64 = s.get(14..16)?.parse().ok()?;
    let second: u64 = s.get(17..19)?.parse().ok()?;
    let days = civil_to_days(year, month, day)?;
    Some(days * 86400 + hour * 3600 + minute * 60 + second)
}

fn civil_to_days(year: u64, month: u64, day: u64) -> Option<u64> {
    let (y, m) = if month <= 2 {
        (year.checked_sub(1)?, month + 9)
    } else {
        (year, month - 3)
    };
    let era = y / 400;
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe;
    days.checked_sub(719468)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_unix_epoch_as_zero() {
        assert_eq!(parse_iso8601_utc_secs("1970-01-01T00:00:00Z"), Some(0));
    }

    #[test]
    fn parses_known_date_to_expected_epoch_seconds() {
        assert_eq!(
            parse_iso8601_utc_secs("2026-07-25T00:00:00Z"),
            Some(20659 * 86400)
        );
    }

    #[test]
    fn parses_date_with_fractional_seconds() {
        assert_eq!(
            parse_iso8601_utc_secs("2026-07-25T00:00:00.000000Z"),
            Some(20659 * 86400)
        );
    }

    #[test]
    fn returns_none_for_invalid_input() {
        assert_eq!(parse_iso8601_utc_secs("not-a-date"), None);
        assert_eq!(parse_iso8601_utc_secs(""), None);
    }

    #[test]
    fn sidecar_load_returns_none_when_file_is_absent() {
        let dir = tempfile::tempdir().unwrap();
        let store = PreferencesSyncStore::new(dir.path().join("preferences_sync.json"));
        assert!(store.load().is_none());
    }

    #[test]
    fn sidecar_record_change_increments_sequence_and_sets_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let store = PreferencesSyncStore::new(dir.path().join("preferences_sync.json"));

        store.record_change(1_000_000).unwrap();
        let meta1 = store.load().unwrap();
        assert_eq!(meta1.sequence, 1);
        assert_eq!(meta1.updated_at_secs, Some(1_000_000));

        store.record_change(2_000_000).unwrap();
        let meta2 = store.load().unwrap();
        assert_eq!(meta2.sequence, 2);
        assert_eq!(meta2.updated_at_secs, Some(2_000_000));
    }

    #[test]
    fn sidecar_record_synced_preserves_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let store = PreferencesSyncStore::new(dir.path().join("preferences_sync.json"));

        store.record_change(1_000_000).unwrap();
        store.record_synced(1_000_001).unwrap();

        let meta = store.load().unwrap();
        assert_eq!(meta.sequence, 1);
        assert_eq!(meta.synced_at_secs, Some(1_000_001));
        assert_eq!(meta.updated_at_secs, Some(1_000_000));
    }

    #[test]
    fn concurrent_save_detected_by_sequence_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let store = PreferencesSyncStore::new(dir.path().join("preferences_sync.json"));

        store.record_change(1_000_000).unwrap();
        let captured_seq = store.load().map(|m| m.sequence).unwrap_or(0);

        store.record_change(2_000_000).unwrap();

        let current_seq = store.load().map(|m| m.sequence).unwrap_or(0);
        assert_ne!(
            current_seq, captured_seq,
            "concurrent save must be detectable via sequence mismatch"
        );
    }
}
