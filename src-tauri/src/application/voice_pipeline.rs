use crate::{
    application::VoicePipelinePort,
    domain::{InterpretOutcome, LiveSessionId, SegmentId, SegmentResult, StageDurations, VerbalixError},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::{pin::Pin, time::Duration};

pub struct RemoteVoicePipeline {
    client: Client,
    base_url: String,
    anonymous_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InterpretPayload {
    request_id: String,
    target_language: String,
    audio_base64: String,
    mime_type: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InterpretResponse {
    #[allow(dead_code)]
    request_id: String,
    detected_language: String,
    audio_base64: String,
    stage_ms: StageMsResponse,
}

#[derive(Deserialize)]
struct StageMsResponse {
    stt: u32,
    translate: u32,
    tts: u32,
}

impl RemoteVoicePipeline {
    pub fn new(base_url: impl Into<String>, anonymous_key: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(55))
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
            StatusCode::NOT_FOUND => VerbalixError::VoiceProfileMissing,
            StatusCode::GATEWAY_TIMEOUT => VerbalixError::ProviderTimeout,
            _ => VerbalixError::InterpretationFailed,
        }
    }
}

impl VoicePipelinePort for RemoteVoicePipeline {
    fn interpret<'a>(
        &'a self,
        session_id: LiveSessionId,
        segment_id: SegmentId,
        wav_bytes: Vec<u8>,
        target_language: &'a str,
        token: &'a str,
    ) -> Pin<Box<dyn std::future::Future<Output = InterpretOutcome> + Send + 'a>> {
        Box::pin(async move {
            let payload = InterpretPayload {
                request_id: format!("{}-{}", session_id.0, segment_id.0),
                target_language: target_language.to_owned(),
                audio_base64: STANDARD.encode(&wav_bytes),
                mime_type: "audio/wav",
            };

            let resp = match self
                .client
                .post(format!("{}/functions/v1/interpret", self.base_url))
                .bearer_auth(token)
                .header("apikey", &self.anonymous_key)
                .json(&payload)
                .send()
                .await
            {
                Ok(r) => r,
                Err(_) => {
                    return InterpretOutcome {
                        session_id,
                        segment_id,
                        result: Err(VerbalixError::InterpretationFailed),
                    }
                }
            };

            let status = resp.status();
            if !status.is_success() {
                return InterpretOutcome {
                    session_id,
                    segment_id,
                    result: Err(Self::map_status_error(status)),
                };
            }

            match resp.json::<InterpretResponse>().await {
                Ok(r) => InterpretOutcome {
                    session_id,
                    segment_id,
                    result: Ok(SegmentResult {
                        audio_base64: r.audio_base64,
                        detected_language: r.detected_language,
                        stage_ms: StageDurations {
                            stt: r.stage_ms.stt,
                            translate: r.stage_ms.translate,
                            tts: r.stage_ms.tts,
                        },
                    }),
                },
                Err(_) => InterpretOutcome {
                    session_id,
                    segment_id,
                    result: Err(VerbalixError::InvalidResponse),
                },
            }
        })
    }
}
