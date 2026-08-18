use crate::{
    application::VoiceEnrollmentPort,
    domain::{EnrollmentSample, VerbalixError, VoiceProfileStatus, VoiceProfileView},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::{pin::Pin, time::Duration};
use uuid::Uuid;

pub struct RemoteVoiceEnrollment {
    client: Client,
    base_url: String,
    anonymous_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnrollPayload<'a> {
    request_id: &'a str,
    display_name: &'a str,
    sample_base64: String,
    mime_type: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VoiceProfileResponse {
    voice_profile_id: Uuid,
    status: VoiceProfileStatus,
    display_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeletePayload<'a> {
    voice_profile_id: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusPayload<'a> {
    voice_profile_id: &'a str,
}

impl RemoteVoiceEnrollment {
    pub fn new(base_url: impl Into<String>, anonymous_key: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(70))
            .build()
            .unwrap_or_default();
        Self {
            client,
            base_url: base_url.into(),
            anonymous_key: anonymous_key.into(),
        }
    }

    fn map_status_error(status: StatusCode) -> VerbalixError {
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => VerbalixError::Unauthenticated,
            StatusCode::GATEWAY_TIMEOUT => VerbalixError::ProviderTimeout,
            _ => VerbalixError::EnrollmentFailed,
        }
    }
}

impl VoiceEnrollmentPort for RemoteVoiceEnrollment {
    fn enroll<'a>(
        &'a self,
        sample: &'a EnrollmentSample,
        display_name: &'a str,
        request_id: Uuid,
        token: &'a str,
    ) -> Pin<
        Box<dyn std::future::Future<Output = Result<VoiceProfileView, VerbalixError>> + Send + 'a>,
    > {
        Box::pin(async move {
            let request_id_str = request_id.to_string();
            let payload = EnrollPayload {
                request_id: &request_id_str,
                display_name,
                sample_base64: STANDARD.encode(&sample.wav_bytes),
                mime_type: "audio/wav",
            };

            let resp = self
                .client
                .post(format!("{}/functions/v1/voice-enroll", self.base_url))
                .bearer_auth(token)
                .header("apikey", &self.anonymous_key)
                .json(&payload)
                .send()
                .await
                .map_err(|_| VerbalixError::EnrollmentFailed)?;

            let status = resp.status();
            if !status.is_success() {
                return Err(Self::map_status_error(status));
            }

            resp.json::<VoiceProfileResponse>()
                .await
                .map_err(|_| VerbalixError::InvalidResponse)
                .map(|r| VoiceProfileView {
                    voice_profile_id: r.voice_profile_id,
                    status: r.status,
                    display_name: r.display_name,
                })
        })
    }

    fn delete<'a>(
        &'a self,
        id: Uuid,
        token: &'a str,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), VerbalixError>> + Send + 'a>> {
        Box::pin(async move {
            let id_str = id.to_string();
            let resp = self
                .client
                .post(format!("{}/functions/v1/voice-delete", self.base_url))
                .bearer_auth(token)
                .header("apikey", &self.anonymous_key)
                .json(&DeletePayload {
                    voice_profile_id: &id_str,
                })
                .send()
                .await
                .map_err(|_| VerbalixError::EnrollmentFailed)?;

            let status = resp.status();
            if !status.is_success() {
                return Err(Self::map_status_error(status));
            }

            Ok(())
        })
    }

    fn status<'a>(
        &'a self,
        id: Uuid,
        token: &'a str,
    ) -> Pin<
        Box<
            dyn std::future::Future<Output = Result<Option<VoiceProfileView>, VerbalixError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let id_str = id.to_string();
            let resp = self
                .client
                .post(format!("{}/functions/v1/voice-status", self.base_url))
                .bearer_auth(token)
                .header("apikey", &self.anonymous_key)
                .json(&StatusPayload {
                    voice_profile_id: &id_str,
                })
                .send()
                .await
                .map_err(|_| VerbalixError::EnrollmentFailed)?;

            let status = resp.status();
            if status == StatusCode::NOT_FOUND {
                return Ok(None);
            }
            if !status.is_success() {
                return Err(Self::map_status_error(status));
            }

            resp.json::<VoiceProfileResponse>()
                .await
                .map_err(|_| VerbalixError::InvalidResponse)
                .map(|r| {
                    Some(VoiceProfileView {
                        voice_profile_id: r.voice_profile_id,
                        status: r.status,
                        display_name: r.display_name,
                    })
                })
        })
    }
}
