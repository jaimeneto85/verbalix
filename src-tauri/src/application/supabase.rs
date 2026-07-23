use crate::domain::{AiProvider, TransformRequest, TransformResult, VerbalixError};
use async_trait::async_trait;
use keyring::Entry;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSession {
    pub access_token: String,
    pub refresh_token: String,
}

pub trait SessionRepository: Send + Sync {
    fn load(&self) -> Result<Option<StoredSession>, VerbalixError>;
    fn save(&self, session: &StoredSession) -> Result<(), VerbalixError>;
    fn clear(&self) -> Result<(), VerbalixError>;
}

pub struct KeychainSessionRepository {
    service: String,
    account: String,
}

impl KeychainSessionRepository {
    pub fn new(service: impl Into<String>, account: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            account: account.into(),
        }
    }

    fn entry(&self) -> Result<Entry, VerbalixError> {
        Entry::new(&self.service, &self.account).map_err(|_| VerbalixError::LocalFailure)
    }
}

impl SessionRepository for KeychainSessionRepository {
    fn load(&self) -> Result<Option<StoredSession>, VerbalixError> {
        match self.entry()?.get_password() {
            Ok(value) => serde_json::from_str(&value)
                .map(Some)
                .map_err(|_| VerbalixError::LocalFailure),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(VerbalixError::LocalFailure),
        }
    }

    fn save(&self, session: &StoredSession) -> Result<(), VerbalixError> {
        let serialized = serde_json::to_string(session).map_err(|_| VerbalixError::LocalFailure)?;
        self.entry()?
            .set_password(&serialized)
            .map_err(|_| VerbalixError::LocalFailure)
    }

    fn clear(&self) -> Result<(), VerbalixError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(VerbalixError::LocalFailure),
        }
    }
}

pub struct RemoteTransformer {
    client: Client,
    endpoint: String,
    anonymous_key: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct HistoryItem {
    pub id: uuid::Uuid,
    pub operation: String,
    pub source_text: String,
    pub result_text: String,
    pub source_language: String,
    pub target_language: Option<String>,
    pub created_at: String,
}

#[derive(Serialize)]
struct HistoryInsert<'a> {
    user_id: &'a str,
    operation: &'a str,
    source_text: &'a str,
    result_text: &'a str,
    source_language: &'a str,
    target_language: &'a Option<String>,
    preferences: &'a Option<crate::domain::TransformPreferences>,
}

#[derive(Deserialize)]
struct SupabaseUser {
    id: String,
}

pub struct RemoteHistoryRepository {
    client: Client,
    base_url: String,
    anonymous_key: String,
}

pub struct RemoteAuthRepository {
    client: Client,
    base_url: String,
    anonymous_key: String,
}

impl RemoteAuthRepository {
    pub fn new(base_url: impl Into<String>, anonymous_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            anonymous_key: anonymous_key.into(),
        }
    }

    pub async fn refresh(&self, session: &StoredSession) -> Result<StoredSession, VerbalixError> {
        #[derive(Serialize)]
        struct RefreshRequest<'a> {
            refresh_token: &'a str,
        }
        #[derive(Deserialize)]
        struct RefreshResponse {
            access_token: String,
            refresh_token: String,
        }
        let response: RefreshResponse = self
            .client
            .post(format!(
                "{}/auth/v1/token?grant_type=refresh_token",
                self.base_url
            ))
            .header("apikey", &self.anonymous_key)
            .json(&RefreshRequest {
                refresh_token: &session.refresh_token,
            })
            .send()
            .await
            .map_err(|_| VerbalixError::ProviderRejected)?
            .error_for_status()
            .map_err(|_| VerbalixError::Unauthenticated)?
            .json()
            .await
            .map_err(|_| VerbalixError::InvalidResponse)?;
        Ok(StoredSession {
            access_token: response.access_token,
            refresh_token: response.refresh_token,
        })
    }
}

impl RemoteHistoryRepository {
    pub fn new(base_url: impl Into<String>, anonymous_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            anonymous_key: anonymous_key.into(),
        }
    }

    async fn user(&self, access_token: &str) -> Result<SupabaseUser, VerbalixError> {
        self.client
            .get(format!("{}/auth/v1/user", self.base_url))
            .bearer_auth(access_token)
            .header("apikey", &self.anonymous_key)
            .send()
            .await
            .map_err(|_| VerbalixError::ProviderRejected)?
            .error_for_status()
            .map_err(|_| VerbalixError::Unauthenticated)?
            .json()
            .await
            .map_err(|_| VerbalixError::InvalidResponse)
    }

    pub async fn insert(
        &self,
        request: &TransformRequest,
        result: &TransformResult,
        access_token: &str,
    ) -> Result<(), VerbalixError> {
        let user = self.user(access_token).await?;
        let operation = match request.operation {
            crate::domain::TransformOperation::Translate => "translate",
            crate::domain::TransformOperation::Improve => "improve",
        };
        self.client
            .post(format!("{}/rest/v1/transform_history", self.base_url))
            .bearer_auth(access_token)
            .header("apikey", &self.anonymous_key)
            .header("Prefer", "return=minimal")
            .json(&HistoryInsert {
                user_id: &user.id,
                operation,
                source_text: &request.text,
                result_text: &result.result,
                source_language: &result.source_language,
                target_language: &result.target_language,
                preferences: &request.preferences,
            })
            .send()
            .await
            .map_err(|_| VerbalixError::ProviderRejected)?
            .error_for_status()
            .map_err(|_| VerbalixError::ProviderRejected)?;
        Ok(())
    }

    pub async fn list(&self, access_token: &str) -> Result<Vec<HistoryItem>, VerbalixError> {
        self.client
            .get(format!(
                "{}/rest/v1/transform_history?select=*&order=created_at.desc",
                self.base_url
            ))
            .bearer_auth(access_token)
            .header("apikey", &self.anonymous_key)
            .send()
            .await
            .map_err(|_| VerbalixError::ProviderRejected)?
            .error_for_status()
            .map_err(|_| VerbalixError::ProviderRejected)?
            .json()
            .await
            .map_err(|_| VerbalixError::InvalidResponse)
    }

    pub async fn delete(
        &self,
        id: Option<uuid::Uuid>,
        access_token: &str,
    ) -> Result<(), VerbalixError> {
        let filter = id
            .map(|id| format!("?id=eq.{id}"))
            .unwrap_or_else(|| "?id=not.is.null".to_owned());
        self.client
            .delete(format!(
                "{}/rest/v1/transform_history{filter}",
                self.base_url
            ))
            .bearer_auth(access_token)
            .header("apikey", &self.anonymous_key)
            .send()
            .await
            .map_err(|_| VerbalixError::ProviderRejected)?
            .error_for_status()
            .map_err(|_| VerbalixError::ProviderRejected)?;
        Ok(())
    }
}

impl RemoteTransformer {
    pub fn new(endpoint: impl Into<String>, anonymous_key: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();
        Self {
            client,
            endpoint: endpoint.into(),
            anonymous_key: anonymous_key.into(),
        }
    }
}

#[async_trait]
impl AiProvider for RemoteTransformer {
    async fn transform(
        &self,
        request: &TransformRequest,
        access_token: &str,
    ) -> Result<TransformResult, VerbalixError> {
        if access_token.trim().is_empty() {
            return Err(VerbalixError::Unauthenticated);
        }
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(access_token)
            .header("apikey", &self.anonymous_key)
            .json(request)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    VerbalixError::ProviderTimeout
                } else {
                    VerbalixError::ProviderRejected
                }
            })?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(VerbalixError::Unauthenticated);
        }
        if !response.status().is_success() {
            return Err(VerbalixError::ProviderRejected);
        }
        let result = response
            .json::<TransformResult>()
            .await
            .map_err(|_| VerbalixError::InvalidResponse)?;
        if result.request_id != request.request_id || result.result.trim().is_empty() {
            return Err(VerbalixError::InvalidResponse);
        }
        Ok(result)
    }
}
