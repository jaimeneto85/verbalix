use super::*;
use crate::{
    application::{
        AudioPreviewPort, AudioStreamPort, VirtualMicMetrics, VirtualMicOutputPort,
        VoicePipelinePort,
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

struct FakeAudioStream {
    sink: Arc<Mutex<Option<SinkFn>>>,
    stop_called: Arc<AtomicBool>,
    start_fails: bool,
}

impl FakeAudioStream {
    fn new() -> Self {
        Self {
            sink: Arc::new(Mutex::new(None)),
            stop_called: Arc::new(AtomicBool::new(false)),
            start_fails: false,
        }
    }

    fn new_failing() -> Self {
        Self {
            sink: Arc::new(Mutex::new(None)),
            stop_called: Arc::new(AtomicBool::new(false)),
            start_fails: true,
        }
    }
}

impl AudioStreamPort for FakeAudioStream {
    fn start_stream(&self, sink: SinkFn, _error_sink: ErrorFn) -> Result<(), VerbalixError> {
        if self.start_fails {
            return Err(VerbalixError::AudioCaptureFailed);
        }
        *self.sink.lock().unwrap() = Some(sink);
        Ok(())
    }

    fn stop_stream(&self) {
        self.stop_called.store(true, Ordering::Relaxed);
    }
}

struct FakePipeline {
    should_fail: bool,
}

impl VoicePipelinePort for FakePipeline {
    fn interpret<'a>(
        &'a self,
        session_id: LiveSessionId,
        segment_id: SegmentId,
        _wav_bytes: Vec<u8>,
        _target_language: &'a str,
        _token: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = InterpretOutcome> + Send + 'a>> {
        let fail = self.should_fail;
        Box::pin(async move {
            if fail {
                InterpretOutcome {
                    session_id,
                    segment_id,
                    result: Err(VerbalixError::InterpretationFailed),
                }
            } else {
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
            }
        })
    }
}

struct FakeAudioPreview;

impl AudioPreviewPort for FakeAudioPreview {
    fn play(&self, _wav_bytes: Vec<u8>) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn stop(&self) {}
}

struct FakeVirtualMicOutput {
    open_fails: bool,
    close_called: Arc<AtomicBool>,
}

impl FakeVirtualMicOutput {
    fn new() -> Self {
        Self {
            open_fails: false,
            close_called: Arc::new(AtomicBool::new(false)),
        }
    }

    fn new_failing() -> Self {
        Self {
            open_fails: true,
            close_called: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl VirtualMicOutputPort for FakeVirtualMicOutput {
    fn open(&self) -> Result<(), VerbalixError> {
        if self.open_fails {
            Err(VerbalixError::VirtualMicUnavailable)
        } else {
            Ok(())
        }
    }

    fn enqueue(&self, _samples_48k: Vec<f32>, _channels: u16) {}

    fn close(&self) {
        self.close_called.store(true, Ordering::Relaxed);
    }

    fn metrics(&self) -> VirtualMicMetrics {
        VirtualMicMetrics {
            buffer_depth: 0,
            underruns: 0,
        }
    }
}

fn make_coordinator(
    capture: Arc<dyn AudioStreamPort>,
    pipeline: Arc<dyn VoicePipelinePort>,
) -> LiveInterpretationCoordinator {
    let route = Arc::new(AtomicBool::new(false));
    LiveInterpretationCoordinator::new(
        pipeline,
        capture,
        Arc::new(FakeAudioPreview),
        Arc::new(RuntimePause::default()),
        Arc::new(|_| {}),
        Arc::new(FakeVirtualMicOutput::new()),
        route,
    )
}

#[test]
fn enter_live_fails_with_unsupported_language() {
    let coord = make_coordinator(
        Arc::new(FakeAudioStream::new()),
        Arc::new(FakePipeline { should_fail: false }),
    );
    let result = coord.enter_live("xx", uuid::Uuid::new_v4(), "tok".to_owned(), false);
    assert!(matches!(
        result,
        Err(VerbalixError::TargetLanguageUnsupported)
    ));
}

#[test]
fn enter_live_starts_capture_stream() {
    let stream = Arc::new(FakeAudioStream::new());
    let coord = make_coordinator(
        Arc::clone(&stream) as Arc<dyn AudioStreamPort>,
        Arc::new(FakePipeline { should_fail: false }),
    );
    let guard = coord.enter_live("en", uuid::Uuid::new_v4(), "tok".to_owned(), false);
    assert!(guard.is_ok());
    assert!(stream.sink.lock().unwrap().is_some());
    assert_eq!(coord.live_state(), LiveState::OnAir);
}

#[test]
fn enter_live_fails_when_capture_fails() {
    let stream = Arc::new(FakeAudioStream::new_failing());
    let coord = make_coordinator(
        Arc::clone(&stream) as Arc<dyn AudioStreamPort>,
        Arc::new(FakePipeline { should_fail: false }),
    );
    let result = coord.enter_live("en", uuid::Uuid::new_v4(), "tok".to_owned(), false);
    assert!(matches!(result, Err(VerbalixError::AudioCaptureFailed)));
    assert_eq!(coord.live_state(), LiveState::Idle);
}

#[test]
fn active_session_id_present_after_enter_live() {
    let coord = make_coordinator(
        Arc::new(FakeAudioStream::new()),
        Arc::new(FakePipeline { should_fail: false }),
    );
    assert!(coord.active_session_id().is_none());
    let _guard = coord
        .enter_live("en", uuid::Uuid::new_v4(), "tok".to_owned(), false)
        .unwrap();
    assert!(coord.active_session_id().is_some());
    coord.leave_live();
    assert!(coord.active_session_id().is_none());
}

#[test]
fn leave_live_stops_capture() {
    let stream = Arc::new(FakeAudioStream::new());
    let coord = make_coordinator(
        Arc::clone(&stream) as Arc<dyn AudioStreamPort>,
        Arc::new(FakePipeline { should_fail: false }),
    );
    let _guard = coord
        .enter_live("en", uuid::Uuid::new_v4(), "tok".to_owned(), false)
        .unwrap();
    coord.leave_live();
    assert!(stream.stop_called.load(Ordering::Relaxed));
    assert_eq!(coord.live_state(), LiveState::Idle);
}

#[test]
fn session_staleness_rejects_old_session() {
    use crate::domain::LiveSession;

    let lang = crate::domain::LanguageTag::parse("en").unwrap();
    let session = LiveSession::new(lang);
    let stale_id = LiveSessionId(uuid::Uuid::new_v4());
    assert!(!session.accepts(stale_id, SegmentId(0)));
    assert!(session.accepts(session.id, SegmentId(0)));
}

#[test]
fn circuit_breaker_triggers_after_k_failures() {
    let coord = make_coordinator(
        Arc::new(FakeAudioStream::new()),
        Arc::new(FakePipeline { should_fail: true }),
    );
    let _guard = coord
        .enter_live("en", uuid::Uuid::new_v4(), "tok".to_owned(), false)
        .unwrap();

    coord.simulate_segment_failure();
    coord.simulate_segment_failure();
    assert_eq!(coord.live_state(), LiveState::Recovering);

    coord.simulate_segment_failure();
    assert_eq!(coord.live_state(), LiveState::Failed);
}

#[test]
fn resolve_route_false_when_setting_off() {
    use crate::application::live_session_setup::LiveEventFn;

    let vmic = FakeVirtualMicOutput::new();
    let events: std::sync::Arc<Mutex<Vec<String>>> = std::sync::Arc::new(Mutex::new(Vec::new()));
    let events_clone = std::sync::Arc::clone(&events);
    let on_event: LiveEventFn = std::sync::Arc::new(move |p: LiveEventPayload| {
        events_clone.lock().unwrap().push(p.status.clone());
    });

    let route = Arc::new(AtomicBool::new(false));
    let coord = LiveInterpretationCoordinator::new(
        Arc::new(FakePipeline { should_fail: false }),
        Arc::new(FakeAudioStream::new()),
        Arc::new(FakeAudioPreview),
        Arc::new(RuntimePause::default()),
        on_event,
        Arc::new(vmic),
        Arc::clone(&route),
    );

    let _ = coord
        .enter_live("en", uuid::Uuid::new_v4(), "tok".to_owned(), false)
        .unwrap();
    assert!(!route.load(Ordering::Relaxed));
    let ev = events.lock().unwrap();
    assert!(!ev.iter().any(|s| s == "virtual-mic-fallback"));
}

#[test]
fn resolve_route_fail_open_when_open_fails() {
    use crate::application::live_session_setup::LiveEventFn;

    let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);
    let on_event: LiveEventFn = Arc::new(move |p: LiveEventPayload| {
        events_clone.lock().unwrap().push(p.status.clone());
    });

    let route = Arc::new(AtomicBool::new(false));
    let coord = LiveInterpretationCoordinator::new(
        Arc::new(FakePipeline { should_fail: false }),
        Arc::new(FakeAudioStream::new()),
        Arc::new(FakeAudioPreview),
        Arc::new(RuntimePause::default()),
        on_event,
        Arc::new(FakeVirtualMicOutput::new_failing()),
        Arc::clone(&route),
    );

    let _ = coord
        .enter_live("en", uuid::Uuid::new_v4(), "tok".to_owned(), true)
        .unwrap();
    assert!(!route.load(Ordering::Relaxed));
    let ev = events.lock().unwrap();
    assert!(ev.contains(&"virtual-mic-fallback".to_owned()));
}

#[test]
fn leave_live_closes_virtual_mic() {
    let close_called = Arc::new(AtomicBool::new(false));
    let vmic = FakeVirtualMicOutput {
        open_fails: false,
        close_called: Arc::clone(&close_called),
    };
    let route = Arc::new(AtomicBool::new(false));
    let coord = LiveInterpretationCoordinator::new(
        Arc::new(FakePipeline { should_fail: false }),
        Arc::new(FakeAudioStream::new()),
        Arc::new(FakeAudioPreview),
        Arc::new(RuntimePause::default()),
        Arc::new(|_| {}),
        Arc::new(vmic),
        Arc::clone(&route),
    );

    let _guard = coord
        .enter_live("en", uuid::Uuid::new_v4(), "tok".to_owned(), false)
        .unwrap();
    coord.leave_live();

    assert!(close_called.load(Ordering::Relaxed));
}
