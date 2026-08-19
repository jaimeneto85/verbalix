use super::*;
use crate::{
    application::{
        live_queue::LiveQueue, streaming_audio::StreamSegmentHandle, AudioPreviewPort,
        VoicePipelinePort,
    },
    domain::{
        InterpretOutcome, LiveSessionId, SegmentId, SegmentResult, StageDurations,
        TranslationContext, VerbalixError,
    },
};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

struct ContextStreamPipeline {
    source_text: String,
}

impl VoicePipelinePort for ContextStreamPipeline {
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
                result: Err(VerbalixError::InterpretationFailed),
            }
        })
    }

    fn interpret_stream<'a>(
        &'a self,
        _session_id: LiveSessionId,
        _segment_id: SegmentId,
        _wav_bytes: Vec<u8>,
        _target_language: &'a str,
        _token: &'a str,
        _context: Vec<String>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<StreamSegmentHandle, InterpretOutcome>>
                + Send
                + 'a,
        >,
    > {
        let src = self.source_text.clone();
        Box::pin(async move {
            let buffer = Arc::new(Mutex::new(VecDeque::from(vec![0.0f32; 8])));
            let complete = Arc::new(AtomicBool::new(true));
            let cancel = Arc::new(AtomicBool::new(false));
            Ok(StreamSegmentHandle {
                buffer,
                complete,
                cancel,
                detected_language: "en".to_owned(),
                target_language: "pt".to_owned(),
                stt_ms: 0,
                translate_ms: 0,
                source_text: src,
            })
        })
    }
}

struct OkPlayback {
    play_count: Arc<AtomicUsize>,
}

impl AudioPreviewPort for OkPlayback {
    fn play(&self, _wav_bytes: Vec<u8>) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn stop(&self) {}

    fn play_stream(&self, _handle: StreamSegmentHandle) -> Result<(), VerbalixError> {
        self.play_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

struct FailPlayback;

impl AudioPreviewPort for FailPlayback {
    fn play(&self, _wav_bytes: Vec<u8>) -> Result<(), VerbalixError> {
        Ok(())
    }

    fn stop(&self) {}

    fn play_stream(&self, _handle: StreamSegmentHandle) -> Result<(), VerbalixError> {
        Err(VerbalixError::AudioCaptureFailed)
    }
}

fn session() -> LiveSessionId {
    LiveSessionId(uuid::Uuid::nil())
}

fn always_accepts() -> Arc<dyn Fn(LiveSessionId, SegmentId) -> bool + Send + Sync + 'static> {
    Arc::new(|_, _| true)
}

fn dispatch_with_ctx(session_id: LiveSessionId, segment_id: SegmentId) -> WorkerCommand {
    WorkerCommand::Dispatch {
        session_id,
        segment_id,
        wav_bytes: vec![],
        target_language: "pt".to_owned(),
        token: "tok".to_owned(),
        t_capture_end: Instant::now(),
        context: vec![],
    }
}

#[test]
fn successful_playback_promotes_context() {
    let ctx = Arc::new(Mutex::new(TranslationContext::new()));
    let pipeline = Arc::new(ContextStreamPipeline {
        source_text: "hello world".to_owned(),
    });
    let playback = Arc::new(OkPlayback {
        play_count: Arc::new(AtomicUsize::new(0)),
    });
    let queue = Arc::new(Mutex::new(LiveQueue::new(8)));

    let worker = LiveWorker::new(
        pipeline,
        playback,
        Arc::clone(&queue),
        Arc::new(|_| {}),
        always_accepts(),
        Arc::clone(&ctx),
    );

    worker.dispatch(dispatch_with_ctx(session(), SegmentId(0)));
    std::thread::sleep(Duration::from_millis(400));

    let snap = ctx.lock().unwrap().snapshot();
    assert_eq!(snap.len(), 1, "successful playback must promote context");
    assert_eq!(snap[0], "hello world");
}

#[test]
fn failed_playback_does_not_promote_context() {
    let ctx = Arc::new(Mutex::new(TranslationContext::new()));
    let pipeline = Arc::new(ContextStreamPipeline {
        source_text: "secret text".to_owned(),
    });
    let playback = Arc::new(FailPlayback);
    let queue = Arc::new(Mutex::new(LiveQueue::new(8)));

    let worker = LiveWorker::new(
        pipeline,
        playback,
        Arc::clone(&queue),
        Arc::new(|_| {}),
        always_accepts(),
        Arc::clone(&ctx),
    );

    worker.dispatch(dispatch_with_ctx(session(), SegmentId(0)));
    std::thread::sleep(Duration::from_millis(400));

    let snap = ctx.lock().unwrap().snapshot();
    assert!(
        snap.is_empty(),
        "failed playback must not promote context (phantom context)"
    );
}

#[test]
fn session_switch_before_promotion_does_not_promote_context() {
    let ctx = Arc::new(Mutex::new(TranslationContext::new()));
    let old_session = session();
    let new_session = LiveSessionId(uuid::Uuid::new_v4());

    let accepted = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let accepted_clone = Arc::clone(&accepted);

    let accepts_fn: Arc<dyn Fn(LiveSessionId, SegmentId) -> bool + Send + Sync + 'static> =
        Arc::new(move |sid: LiveSessionId, _| {
            if sid == old_session {
                accepted_clone.load(Ordering::Relaxed)
            } else {
                true
            }
        });

    let pipeline = Arc::new(ContextStreamPipeline {
        source_text: "old session text".to_owned(),
    });
    let playback = Arc::new(OkPlayback {
        play_count: Arc::new(AtomicUsize::new(0)),
    });
    let queue = Arc::new(Mutex::new(LiveQueue::new(8)));

    let worker = LiveWorker::new(
        Arc::clone(&pipeline) as Arc<dyn VoicePipelinePort>,
        playback,
        Arc::clone(&queue),
        Arc::new(|_| {}),
        accepts_fn,
        Arc::clone(&ctx),
    );

    accepted.store(false, Ordering::Relaxed);
    worker.dispatch(WorkerCommand::Dispatch {
        session_id: old_session,
        segment_id: SegmentId(0),
        wav_bytes: vec![],
        target_language: "pt".to_owned(),
        token: "tok".to_owned(),
        t_capture_end: Instant::now(),
        context: vec![],
    });
    std::thread::sleep(Duration::from_millis(400));

    let snap = ctx.lock().unwrap().snapshot();
    assert!(
        snap.is_empty(),
        "session switch before promotion must not promote context"
    );
}

#[test]
fn emitted_events_carry_no_source_text() {
    let ctx = Arc::new(Mutex::new(TranslationContext::new()));
    let pipeline = Arc::new(ContextStreamPipeline {
        source_text: "private-data-xyz".to_owned(),
    });
    let playback = Arc::new(OkPlayback {
        play_count: Arc::new(AtomicUsize::new(0)),
    });
    let queue = Arc::new(Mutex::new(LiveQueue::new(8)));

    let event_log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let event_log_clone = Arc::clone(&event_log);

    let worker = LiveWorker::new(
        pipeline,
        playback,
        Arc::clone(&queue),
        Arc::new(move |event| {
            let label = match &event {
                WorkerEvent::Ready {
                    detected_language, ..
                } => detected_language.clone(),
                WorkerEvent::Failed { .. } => "failed".to_owned(),
                WorkerEvent::Dropped { .. } => "dropped".to_owned(),
            };
            event_log_clone.lock().unwrap().push(label);
        }),
        always_accepts(),
        Arc::clone(&ctx),
    );

    worker.dispatch(dispatch_with_ctx(session(), SegmentId(0)));
    std::thread::sleep(Duration::from_millis(400));

    let log = event_log.lock().unwrap();
    for entry in log.iter() {
        assert!(
            !entry.contains("private-data-xyz"),
            "emitted event must not expose source text"
        );
    }
}

#[test]
fn context_snapshot_forwarded_in_dispatch_command() {
    let mut ctx = TranslationContext::new();
    ctx.push("prior segment");
    let snap = ctx.snapshot();

    let cmd = WorkerCommand::Dispatch {
        session_id: session(),
        segment_id: SegmentId(0),
        wav_bytes: vec![],
        target_language: "pt".to_owned(),
        token: "tok".to_owned(),
        t_capture_end: Instant::now(),
        context: snap.clone(),
    };

    if let WorkerCommand::Dispatch { context, .. } = cmd {
        assert_eq!(context, snap, "dispatch must carry the context snapshot");
    }
}
