use super::supabase::StoredSession;
use crate::domain::VerbalixError;
use reqwest::Client;
use serde::{Deserialize, Serialize};

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
