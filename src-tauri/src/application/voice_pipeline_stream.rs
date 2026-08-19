use crate::{
    application::streaming_audio::StreamSegmentHandle,
    domain::{InterpretOutcome, LiveSessionId, SegmentId, VerbalixError},
    platform::audio_wav::pcm_i16le_to_f32,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::StreamExt;
use reqwest::{Client, Response, StatusCode};
use serde::Deserialize;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    code: String,
}

pub(crate) async fn parse_error_response(resp: Response, status: StatusCode) -> VerbalixError {
    if let Ok(body) = resp.json::<ErrorBody>().await {
        match body.error.code.as_str() {
            "STT_FAILED" => return VerbalixError::SttFailed,
            "TRANSLATION_FAILED" => return VerbalixError::TranslationFailed,
            "TTS_FAILED" => return VerbalixError::TtsFailed,
            _ => {}
        }
    }
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => VerbalixError::Unauthenticated,
        StatusCode::NOT_FOUND => VerbalixError::VoiceProfileMissing,
        StatusCode::GATEWAY_TIMEOUT => VerbalixError::ProviderTimeout,
        _ => VerbalixError::InterpretationFailed,
    }
}

pub(crate) fn drain_pcm_with_remainder(chunk: &[u8], remainder: &mut Option<u8>) -> Vec<f32> {
    let full = match remainder.take() {
        Some(prev) => {
            let mut v = Vec::with_capacity(chunk.len() + 1);
            v.push(prev);
            v.extend_from_slice(chunk);
            v
        }
        None => chunk.to_vec(),
    };
    let total = full.len();
    if total % 2 == 1 {
        *remainder = Some(full[total - 1]);
    }
    pcm_i16le_to_f32(&full[..total & !1])
}

pub(crate) fn parse_vlbx_preamble(buf: &[u8]) -> Option<(usize, String)> {
    if buf.len() < 8 {
        return None;
    }
    if &buf[0..4] != b"VLBX" {
        return None;
    }
    let json_len = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
    let total = 8 + json_len;
    if buf.len() < total {
        return None;
    }
    let source_text = serde_json::from_slice::<serde_json::Value>(&buf[8..total])
        .ok()
        .and_then(|v| {
            v.get("sourceText")
                .and_then(|s| s.as_str())
                .map(String::from)
        })
        .unwrap_or_default();
    Some((total, source_text))
}

pub(crate) async fn drain_streaming_body<S, C, E>(
    mut stream: S,
    initial_pcm: Vec<u8>,
    buffer: Arc<Mutex<VecDeque<f32>>>,
    complete: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
) where
    S: futures_util::Stream<Item = Result<C, E>> + Unpin + Send + 'static,
    C: AsRef<[u8]> + Send + 'static,
    E: Send + 'static,
{
    let mut remainder: Option<u8> = None;

    let samples = drain_pcm_with_remainder(&initial_pcm, &mut remainder);
    if !samples.is_empty() {
        buffer.lock().unwrap().extend(samples);
    }

    while let Some(item) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        match item {
            Ok(chunk) => {
                let samples = drain_pcm_with_remainder(chunk.as_ref(), &mut remainder);
                if !samples.is_empty() {
                    buffer.lock().unwrap().extend(samples);
                }
            }
            Err(_) => break,
        }
    }

    complete.store(true, Ordering::Release);
}

pub(crate) async fn do_interpret_stream(
    client: &Client,
    base_url: &str,
    anonymous_key: &str,
    session_id: LiveSessionId,
    segment_id: SegmentId,
    wav_bytes: Vec<u8>,
    target_language: String,
    token: String,
) -> Result<StreamSegmentHandle, InterpretOutcome> {
    let payload = serde_json::json!({
        "requestId": format!("{}-{}", session_id.0, segment_id.0),
        "targetLanguage": target_language,
        "audioBase64": STANDARD.encode(&wav_bytes),
        "mimeType": "audio/wav",
        "stream": true,
    });

    let resp = match client
        .post(format!("{}/functions/v1/interpret", base_url))
        .bearer_auth(&token)
        .header("apikey", anonymous_key)
        .json(&payload)
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => {
            return Err(InterpretOutcome {
                session_id,
                segment_id,
                result: Err(VerbalixError::InterpretationFailed),
            })
        }
    };

    let status = resp.status();
    if !status.is_success() {
        let error = parse_error_response(resp, status).await;
        return Err(InterpretOutcome {
            session_id,
            segment_id,
            result: Err(error),
        });
    }

    let detected_language = resp
        .headers()
        .get("x-verbalix-detected-language")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let resp_target = resp
        .headers()
        .get("x-verbalix-target-language")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(&target_language)
        .to_owned();
    let stt_ms = resp
        .headers()
        .get("x-verbalix-stt-ms")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let translate_ms = resp
        .headers()
        .get("x-verbalix-translate-ms")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    let mut stream = resp.bytes_stream();
    let mut preamble_buf: Vec<u8> = Vec::new();

    let header_result = loop {
        if let Some(pair) = parse_vlbx_preamble(&preamble_buf) {
            break Some(pair);
        }
        match stream.next().await {
            Some(Ok(chunk)) => preamble_buf.extend_from_slice(&chunk),
            _ => break None,
        }
    };

    let (header_len, source_text) = match header_result {
        Some(v) => v,
        None => {
            return Err(InterpretOutcome {
                session_id,
                segment_id,
                result: Err(VerbalixError::InterpretationFailed),
            })
        }
    };

    let initial_pcm = preamble_buf[header_len..].to_vec();
    let buffer: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));
    let complete = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(AtomicBool::new(false));

    tokio::spawn(drain_streaming_body(
        stream,
        initial_pcm,
        buffer.clone(),
        complete.clone(),
        cancel.clone(),
    ));

    Ok(StreamSegmentHandle {
        buffer,
        complete,
        cancel,
        detected_language,
        target_language: resp_target,
        stt_ms,
        translate_ms,
        source_text,
    })
}
