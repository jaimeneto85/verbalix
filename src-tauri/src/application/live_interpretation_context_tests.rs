use super::*;
use crate::{
    application::{
        live_session_setup::LiveEventPayload, AudioPreviewPort, AudioStreamPort, VirtualMicMetrics,
        VirtualMicOutputPort, VoicePipelinePort,
    },
    domain::{
        InterpretOutcome, LiveSessionId, SegmentId, SegmentResult, StageDurations, VerbalixError,
    },
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

type SinkFn = Box<dyn Fn(Vec<f32>, u32, u16) + Send + Sync + 'static>;
type ErrorFn = Box<dyn Fn() + Send + Sync + 'static>;

struct StubStream {
    stop_called: Arc<AtomicBool>,
}

impl StubStream {
    fn new() -> Self {
        Self {
            stop_called: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl AudioStreamPort for StubStream {
    fn start_stream(&self, _sink: SinkFn, _error_sink: ErrorFn) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn stop_stream(&self) {
        self.stop_called.store(true, Ordering::Relaxed);
    }
}

struct StubPipeline;

impl VoicePipelinePort for StubPipeline {
    fn interpret<'a>(
        &'a self,
        session_id: LiveSessionId,
        segment_id: SegmentId,
        _wav_bytes: Vec<u8>,
        _target_language: &'a str,
        _token: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = InterpretOutcome> + Send + 'a>> {
        Box::pin(async move {
            InterpretOutcome {
                session_id,
                segment_id,
                result: Ok(SegmentResult {
                    audio_base64: String::new(),
                    detected_language: "en".to_owned(),
                    stage_ms: StageDurations {
                        stt: 0,
                        translate: 0,
                        tts: 0,
                    },
                }),
            }
        })
    }
}

struct StubPreview;

impl AudioPreviewPort for StubPreview {
    fn play(&self, _wav_bytes: Vec<u8>) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn stop(&self) {}
}

struct StubVmic;

impl VirtualMicOutputPort for StubVmic {
    fn open(&self) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn enqueue(&self, _samples_48k: Vec<f32>, _channels: u16) {}

    fn close(&self) {}

    fn metrics(&self) -> VirtualMicMetrics {
        VirtualMicMetrics {
            buffer_depth: 0,
            underruns: 0,
        }
    }
}

fn make_coord() -> LiveInterpretationCoordinator {
    LiveInterpretationCoordinator::new(
        Arc::new(StubPipeline),
        Arc::new(StubStream::new()),
        Arc::new(StubPreview),
        Arc::new(RuntimePause::default()),
        Arc::new(|_: LiveEventPayload| {}),
        Arc::new(StubVmic),
        Arc::new(AtomicBool::new(false)),
    )
}

#[test]
fn context_snapshot_empty_before_enter_live() {
    let coord = make_coord();
    assert!(coord.context_snapshot().is_empty());
}

#[test]
fn context_snapshot_empty_after_leave_live() {
    let coord = make_coord();
    let _guard = coord
        .enter_live("en", uuid::Uuid::new_v4(), "tok".to_owned(), false)
        .unwrap();
    coord.leave_live();
    assert!(coord.context_snapshot().is_empty());
}

#[test]
fn live_event_payload_carries_no_source_text() {
    let events: Arc<Mutex<Vec<LiveEventPayload>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);
    let coord = LiveInterpretationCoordinator::new(
        Arc::new(StubPipeline),
        Arc::new(StubStream::new()),
        Arc::new(StubPreview),
        Arc::new(RuntimePause::default()),
        Arc::new(move |p: LiveEventPayload| {
            events_clone.lock().unwrap().push(p);
        }),
        Arc::new(StubVmic),
        Arc::new(AtomicBool::new(false)),
    );

    let _guard = coord
        .enter_live("en", uuid::Uuid::new_v4(), "tok".to_owned(), false)
        .unwrap();
    coord.leave_live();

    let captured = events.lock().unwrap();
    for payload in captured.iter() {
        assert!(
            !payload.status.contains("source"),
            "status must not contain source text"
        );
    }
}
