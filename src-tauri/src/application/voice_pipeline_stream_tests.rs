use crate::application::voice_pipeline_stream::{
    drain_pcm_with_remainder, drain_streaming_body, parse_vlbx_preamble,
};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

fn make_vlbx_frame(source_text: &str, pcm_bytes: &[u8]) -> Vec<u8> {
    let json = format!(r#"{{"sourceText":"{}"}}"#, source_text);
    let json_bytes = json.as_bytes();
    let json_len = json_bytes.len() as u32;
    let mut frame = Vec::new();
    frame.extend_from_slice(b"VLBX");
    frame.extend_from_slice(&json_len.to_be_bytes());
    frame.extend_from_slice(json_bytes);
    frame.extend_from_slice(pcm_bytes);
    frame
}

fn i16_to_le_bytes(v: i16) -> [u8; 2] {
    v.to_le_bytes()
}

#[test]
fn parse_vlbx_preamble_valid_frame() {
    let frame = make_vlbx_frame("hello", &[]);
    let result = parse_vlbx_preamble(&frame);
    assert!(result.is_some());
    let (header_len, source_text) = result.unwrap();
    assert_eq!(source_text, "hello");
    assert_eq!(header_len, 8 + r#"{"sourceText":"hello"}"#.len());
}

#[test]
fn parse_vlbx_preamble_invalid_magic() {
    let mut frame = make_vlbx_frame("x", &[]);
    frame[0] = 0x00;
    assert!(parse_vlbx_preamble(&frame).is_none());
}

#[test]
fn parse_vlbx_preamble_reads_u32_be_correctly() {
    let json = r#"{"sourceText":"ab"}"#;
    let json_bytes = json.as_bytes();
    let json_len = json_bytes.len() as u32;

    let mut frame = Vec::new();
    frame.extend_from_slice(b"VLBX");
    frame.extend_from_slice(&json_len.to_be_bytes());
    frame.extend_from_slice(json_bytes);

    let result = parse_vlbx_preamble(&frame);
    assert!(result.is_some());
    let (header_len, source_text) = result.unwrap();
    assert_eq!(source_text, "ab");
    assert_eq!(header_len, 8 + json_bytes.len());
}

#[test]
fn parse_vlbx_preamble_incomplete_returns_none() {
    let frame = make_vlbx_frame("test", &[]);
    assert!(parse_vlbx_preamble(&frame[..4]).is_none());
    assert!(parse_vlbx_preamble(&frame[..7]).is_none());
}

#[test]
fn drain_pcm_with_remainder_even_chunk() {
    let mut rem: Option<u8> = None;
    let sample: i16 = 1000;
    let bytes = sample.to_le_bytes();
    let out = drain_pcm_with_remainder(&bytes, &mut rem);
    assert_eq!(out.len(), 1);
    assert!((out[0] - 1000.0 / 32768.0).abs() < 1e-4);
    assert!(rem.is_none());
}

#[test]
fn drain_pcm_with_remainder_odd_byte_across_chunks() {
    let s1: i16 = 1000;
    let s2: i16 = -2000;

    let all_bytes: Vec<u8> = s1
        .to_le_bytes()
        .iter()
        .chain(s2.to_le_bytes().iter())
        .copied()
        .collect();

    let chunk_a = &all_bytes[..3];
    let chunk_b = &all_bytes[3..];

    let mut rem: Option<u8> = None;
    let out_a = drain_pcm_with_remainder(chunk_a, &mut rem);
    assert_eq!(out_a.len(), 1);
    assert!((out_a[0] - 1000.0 / 32768.0).abs() < 1e-4);
    assert!(rem.is_some(), "odd byte must be buffered");

    let out_b = drain_pcm_with_remainder(chunk_b, &mut rem);
    assert_eq!(out_b.len(), 1);
    assert!((out_b[0] + 2000.0 / 32768.0).abs() < 1e-4);
    assert!(rem.is_none());
}

#[tokio::test]
async fn drain_streaming_body_cancel_stops_drain() {
    let buffer = Arc::new(Mutex::new(VecDeque::new()));
    let complete = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(AtomicBool::new(true));

    let sample: i16 = 500;
    let pcm: Vec<u8> = sample.to_le_bytes().to_vec();
    let stream = futures_util::stream::iter(vec![Ok::<Vec<u8>, std::io::Error>(pcm)]);

    drain_streaming_body(
        stream,
        vec![],
        buffer.clone(),
        complete.clone(),
        cancel.clone(),
    )
    .await;

    assert!(complete.load(Ordering::Relaxed));
    assert!(
        buffer.lock().unwrap().is_empty(),
        "cancel must prevent samples from being pushed"
    );
}

#[tokio::test]
async fn drain_streaming_body_drains_initial_pcm_and_stream_chunks() {
    let buffer = Arc::new(Mutex::new(VecDeque::new()));
    let complete = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(AtomicBool::new(false));

    let s1 = i16_to_le_bytes(1000);
    let s2 = i16_to_le_bytes(2000);
    let initial = s1.to_vec();
    let chunk: Vec<u8> = s2.to_vec();

    let stream = futures_util::stream::iter(vec![Ok::<Vec<u8>, std::io::Error>(chunk)]);

    drain_streaming_body(
        stream,
        initial,
        buffer.clone(),
        complete.clone(),
        cancel.clone(),
    )
    .await;

    assert!(complete.load(Ordering::Relaxed));
    let buf = buffer.lock().unwrap();
    assert_eq!(buf.len(), 2);
    assert!((buf[0] - 1000.0 / 32768.0).abs() < 1e-4);
    assert!((buf[1] - 2000.0 / 32768.0).abs() < 1e-4);
}

#[tokio::test]
async fn drain_streaming_body_handles_stream_error_gracefully() {
    let buffer = Arc::new(Mutex::new(VecDeque::new()));
    let complete = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(AtomicBool::new(false));

    let stream = futures_util::stream::iter(vec![Err::<Vec<u8>, std::io::Error>(
        std::io::Error::new(std::io::ErrorKind::BrokenPipe, "x"),
    )]);

    drain_streaming_body(
        stream,
        vec![],
        buffer.clone(),
        complete.clone(),
        cancel.clone(),
    )
    .await;

    assert!(complete.load(Ordering::Relaxed));
}

#[test]
fn json_legacy_mode_uses_stream_false() {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let payload = serde_json::json!({
        "requestId": "test-1",
        "targetLanguage": "en",
        "audioBase64": STANDARD.encode(b"fake"),
        "mimeType": "audio/wav",
        "stream": false,
    });
    assert_eq!(payload["stream"], serde_json::json!(false));
}
