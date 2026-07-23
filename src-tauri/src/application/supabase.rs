use crate::domain::{
    AiProvider, TransformRequest, TransformResult, VerbalixError,
};
use async_trait::async_trait;
use keyring::Entry;
use reqwest::{Client, StatusCode};
use std::time::Duration;

pub trait SessionRepository: Send + Sync {
    fn load(&self) -> Result<Option<String>, VerbalixError>;
    fn save(&self, token: &str) -> Result<(), VerbalixError>;
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
    fn load(&self) -> Result<Option<String>, VerbalixError> {
        match self.entry()?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(VerbalixError::LocalFailure),
        }
    }

    fn save(&self, token: &str) -> Result<(), VerbalixError> {
        self.entry()?
            .set_password(token)
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
