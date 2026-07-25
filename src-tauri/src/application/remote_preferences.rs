use crate::{
    application::preferences_sync_store::{parse_iso8601_utc_secs, LocalSyncMeta},
    domain::{AppSettings, LengthPreference, TonePreference, VerbalixError},
};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct RemotePreferencesRepository {
    client: Client,
    base_url: String,
    anonymous_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RemotePreferences {
    pub formality: u8,
    pub length: LengthPreference,
    pub tone: TonePreference,
    pub history_enabled: bool,
    pub updated_at: Option<String>,
}

#[derive(Serialize)]
struct PreferencesUpsert {
    user_id: String,
    formality: u8,
    length: String,
    tone: String,
    history_enabled: bool,
}

#[derive(Deserialize)]
struct SupabaseUser {
    id: String,
}

pub struct MergeOutcome {
    pub settings: AppSettings,
    pub needs_push: bool,
}

impl RemotePreferencesRepository {
    pub fn new(base_url: impl Into<String>, anonymous_key: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(4))
            .build()
            .unwrap_or_default();
        Self {
            client,
            base_url: base_url.into(),
            anonymous_key: anonymous_key.into(),
        }
    }

    async fn user_id(&self, access_token: &str) -> Result<String, VerbalixError> {
        self.client
            .get(format!("{}/auth/v1/user", self.base_url))
            .bearer_auth(access_token)
            .header("apikey", &self.anonymous_key)
            .send()
            .await
            .map_err(|_| VerbalixError::ProviderRejected)?
            .error_for_status()
            .map_err(|_| VerbalixError::Unauthenticated)?
            .json::<SupabaseUser>()
            .await
            .map(|u| u.id)
            .map_err(|_| VerbalixError::InvalidResponse)
    }

    pub async fn fetch(
        &self,
        access_token: &str,
    ) -> Result<Option<RemotePreferences>, VerbalixError> {
        let response = self
            .client
            .get(format!(
                "{}/rest/v1/user_preferences?select=*&limit=1",
                self.base_url
            ))
            .bearer_auth(access_token)
            .header("apikey", &self.anonymous_key)
            .send()
            .await
            .map_err(|_| VerbalixError::ProviderRejected)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(VerbalixError::Unauthenticated);
        }
        response
            .error_for_status()
            .map_err(|_| VerbalixError::ProviderRejected)?
            .json::<Vec<RemotePreferences>>()
            .await
            .map(|rows: Vec<RemotePreferences>| rows.into_iter().next())
            .map_err(|_| VerbalixError::InvalidResponse)
    }

    pub async fn upsert(
        &self,
        settings: &AppSettings,
        access_token: &str,
    ) -> Result<(), VerbalixError> {
        let user_id = self.user_id(access_token).await?;
        let length = length_str(settings.length);
        let tone = tone_str(settings.tone);
        self.client
            .post(format!(
                "{}/rest/v1/user_preferences?on_conflict=user_id",
                self.base_url
            ))
            .bearer_auth(access_token)
            .header("apikey", &self.anonymous_key)
            .header("Prefer", "resolution=merge-duplicates,return=minimal")
            .json(&PreferencesUpsert {
                user_id,
                formality: settings.formality,
                length,
                tone,
                history_enabled: settings.history_enabled,
            })
            .send()
            .await
            .map_err(|_| VerbalixError::ProviderRejected)?
            .error_for_status()
            .map_err(|_| VerbalixError::ProviderRejected)?;
        Ok(())
    }
}

fn length_str(length: LengthPreference) -> String {
    match length {
        LengthPreference::Concise => "concise",
        LengthPreference::Balanced => "balanced",
        LengthPreference::Detailed => "detailed",
    }
    .to_owned()
}

fn tone_str(tone: TonePreference) -> String {
    match tone {
        TonePreference::Neutral => "neutral",
        TonePreference::Friendly => "friendly",
        TonePreference::Assertive => "assertive",
        TonePreference::Technical => "technical",
    }
    .to_owned()
}

pub fn merge_preferences(
    local: &AppSettings,
    local_meta: Option<&LocalSyncMeta>,
    remote: Option<RemotePreferences>,
) -> MergeOutcome {
    let Some(remote) = remote else {
        return MergeOutcome {
            settings: local.clone(),
            needs_push: false,
        };
    };

    let local_updated_at_secs = local_meta.and_then(|m| m.updated_at_secs);
    let synced_at_secs = local_meta.and_then(|m| m.synced_at_secs);

    let remote_secs = remote
        .updated_at
        .as_deref()
        .and_then(parse_iso8601_utc_secs);

    let Some(remote_secs) = remote_secs else {
        let needs_push = should_push(local_updated_at_secs, synced_at_secs);
        return MergeOutcome {
            settings: local.clone(),
            needs_push,
        };
    };

    let Some(local_secs) = local_updated_at_secs else {
        return MergeOutcome {
            settings: apply_remote(remote, local),
            needs_push: false,
        };
    };

    if remote_secs > local_secs {
        MergeOutcome {
            settings: apply_remote(remote, local),
            needs_push: false,
        }
    } else {
        let needs_push = should_push(Some(local_secs), synced_at_secs);
        MergeOutcome {
            settings: local.clone(),
            needs_push,
        }
    }
}

fn should_push(local_updated_at_secs: Option<u64>, synced_at_secs: Option<u64>) -> bool {
    match (local_updated_at_secs, synced_at_secs) {
        (Some(updated), Some(synced)) => updated > synced,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

fn apply_remote(remote: RemotePreferences, local: &AppSettings) -> AppSettings {
    AppSettings {
        formality: remote.formality,
        length: remote.length,
        tone: remote.tone,
        history_enabled: remote.history_enabled,
        confirm_before_replace: local.confirm_before_replace,
        automatic_toolbar: local.automatic_toolbar,
        shortcut: local.shortcut.clone(),
    }
}

#[cfg(test)]
#[path = "remote_preferences_tests.rs"]
mod remote_preferences_tests;
