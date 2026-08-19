use crate::{
    application::{
        streaming_audio::StreamSegmentHandle,
        voice_pipeline_stream::{do_interpret_stream, parse_error_response, StreamRequest},
        VoicePipelinePort,
    },
    domain::{
        InterpretOutcome, LiveSessionId, SegmentId, SegmentResult, StageDurations, VerbalixError,
    },
};
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::{pin::Pin, time::Duration};

pub struct RemoteVoicePipeline {
    pub(crate) client: Client,
    pub(crate) base_url: String,
    pub(crate) anonymous_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InterpretJsonPayload {
    request_id: String,
    target_language: String,
    audio_base64: String,
    mime_type: &'static str,
    stream: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InterpretResponseBody {
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

    async fn parse_error(resp: Response, status: StatusCode) -> VerbalixError {
        parse_error_response(resp, status).await
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
            let payload = InterpretJsonPayload {
                request_id: format!("{}-{}", session_id.0, segment_id.0),
                target_language: target_language.to_owned(),
                audio_base64: STANDARD.encode(&wav_bytes),
                mime_type: "audio/wav",
                stream: false,
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
                let error = Self::parse_error(resp, status).await;
                return InterpretOutcome {
                    session_id,
                    segment_id,
                    result: Err(error),
                };
            }

            match resp.json::<InterpretResponseBody>().await {
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

    fn interpret_stream<'a>(
        &'a self,
        session_id: LiveSessionId,
        segment_id: SegmentId,
        wav_bytes: Vec<u8>,
        target_language: &'a str,
        token: &'a str,
    ) -> Pin<
        Box<
            dyn std::future::Future<Output = Result<StreamSegmentHandle, InterpretOutcome>>
                + Send
                + 'a,
        >,
    > {
        let target = target_language.to_owned();
        let tok = token.to_owned();
        Box::pin(async move {
            do_interpret_stream(
                &self.client,
                &self.base_url,
                &self.anonymous_key,
                StreamRequest {
                    session_id,
                    segment_id,
                    wav_bytes,
                    target_language: target,
                    token: tok,
                },
            )
            .await
        })
    }
}
