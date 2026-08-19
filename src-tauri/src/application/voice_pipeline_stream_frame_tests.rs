use crate::application::voice_pipeline_stream::{drain_pcm_with_remainder, parse_vlbx_preamble};

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
fn parse_vlbx_preamble_embedded_pcm_extracts_initial_pcm() {
    let pcm_sample: i16 = 3000;
    let pcm_bytes = pcm_sample.to_le_bytes();
    let frame = make_vlbx_frame("world", &pcm_bytes);
    let result = parse_vlbx_preamble(&frame);
    assert!(result.is_some());
    let (header_len, source_text) = result.unwrap();
    assert_eq!(source_text, "world");
    let initial_pcm = &frame[header_len..];
    assert_eq!(initial_pcm, pcm_bytes.as_slice());
    let mut rem: Option<u8> = None;
    let samples = drain_pcm_with_remainder(initial_pcm, &mut rem);
    assert_eq!(samples.len(), 1);
    assert!((samples[0] - 3000.0 / 32768.0).abs() < 1e-4);
}

#[test]
fn parse_vlbx_preamble_invalid_magic_second_byte() {
    let mut frame = make_vlbx_frame("x", &[]);
    frame[1] = 0x00;
    assert!(parse_vlbx_preamble(&frame).is_none());
}

#[test]
fn parse_vlbx_preamble_invalid_magic_third_byte() {
    let mut frame = make_vlbx_frame("x", &[]);
    frame[2] = 0x00;
    assert!(parse_vlbx_preamble(&frame).is_none());
}

#[test]
fn parse_vlbx_preamble_invalid_magic_fourth_byte() {
    let mut frame = make_vlbx_frame("x", &[]);
    frame[3] = 0x00;
    assert!(parse_vlbx_preamble(&frame).is_none());
}

#[test]
fn parse_vlbx_preamble_zero_json_len_returns_empty_source_text() {
    let mut frame = Vec::new();
    frame.extend_from_slice(b"VLBX");
    frame.extend_from_slice(&0u32.to_be_bytes());
    let result = parse_vlbx_preamble(&frame);
    assert!(result.is_some());
    let (header_len, source_text) = result.unwrap();
    assert_eq!(header_len, 8);
    assert_eq!(source_text, "");
}

#[test]
fn parse_vlbx_preamble_accumulation_across_three_tiny_chunks() {
    let json = r#"{"sourceText":"split"}"#;
    let json_bytes = json.as_bytes();
    let json_len = json_bytes.len() as u32;
    let mut full_frame = Vec::new();
    full_frame.extend_from_slice(b"VLBX");
    full_frame.extend_from_slice(&json_len.to_be_bytes());
    full_frame.extend_from_slice(json_bytes);
    let mut buf = Vec::new();
    buf.extend_from_slice(&full_frame[..4]);
    assert!(parse_vlbx_preamble(&buf).is_none());
    buf.extend_from_slice(&full_frame[4..8]);
    assert!(parse_vlbx_preamble(&buf).is_none());
    buf.extend_from_slice(&full_frame[8..]);
    let result = parse_vlbx_preamble(&buf);
    assert!(result.is_some());
    let (header_len, source_text) = result.unwrap();
    assert_eq!(source_text, "split");
    assert_eq!(header_len, 8 + json_bytes.len());
}
