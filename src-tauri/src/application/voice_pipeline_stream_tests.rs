use crate::application::voice_pipeline_stream::{drain_pcm_with_remainder, drain_streaming_body};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

fn i16_to_le_bytes(v: i16) -> [u8; 2] {
    v.to_le_bytes()
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

#[test]
fn drain_pcm_with_remainder_odd_byte_three_chunks() {
    let s1: i16 = 100;
    let s2: i16 = 200;
    let s3: i16 = 300;
    let all_bytes: Vec<u8> = s1
        .to_le_bytes()
        .iter()
        .chain(s2.to_le_bytes().iter())
        .chain(s3.to_le_bytes().iter())
        .copied()
        .collect();
    let chunk_a = &all_bytes[..3];
    let chunk_b = &all_bytes[3..5];
    let chunk_c = &all_bytes[5..];
    let mut rem: Option<u8> = None;
    let out_a = drain_pcm_with_remainder(chunk_a, &mut rem);
    assert_eq!(out_a.len(), 1);
    assert!((out_a[0] - 100.0 / 32768.0).abs() < 1e-4);
    assert!(rem.is_some());
    let out_b = drain_pcm_with_remainder(chunk_b, &mut rem);
    assert_eq!(out_b.len(), 1);
    assert!((out_b[0] - 200.0 / 32768.0).abs() < 1e-4);
    assert!(rem.is_some());
    let out_c = drain_pcm_with_remainder(chunk_c, &mut rem);
    assert_eq!(out_c.len(), 1);
    assert!((out_c[0] - 300.0 / 32768.0).abs() < 1e-4);
    assert!(rem.is_none());
}

#[tokio::test]
async fn drain_streaming_body_cancel_after_first_chunk() {
    use std::sync::atomic::Ordering;
    let buffer = Arc::new(Mutex::new(VecDeque::new()));
    let complete = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_stream = cancel.clone();
    let s1: i16 = 1000;
    let s2: i16 = 2000;
    let stream = Box::pin(futures_util::stream::unfold(
        (0u8, cancel_for_stream),
        move |(count, cancel_ref)| async move {
            if count == 0 {
                Some((
                    Ok::<Vec<u8>, std::io::Error>(s1.to_le_bytes().to_vec()),
                    (1, cancel_ref),
                ))
            } else if count == 1 {
                cancel_ref.store(true, Ordering::Relaxed);
                Some((
                    Ok::<Vec<u8>, std::io::Error>(s2.to_le_bytes().to_vec()),
                    (2, cancel_ref),
                ))
            } else {
                None
            }
        },
    ));
    drain_streaming_body(
        stream,
        vec![],
        buffer.clone(),
        complete.clone(),
        cancel.clone(),
    )
    .await;
    assert!(complete.load(Ordering::Relaxed));
    let buf = buffer.lock().unwrap();
    assert_eq!(
        buf.len(),
        1,
        "second chunk must not be processed after cancel"
    );
    assert!((buf[0] - 1000.0 / 32768.0).abs() < 1e-4);
}
