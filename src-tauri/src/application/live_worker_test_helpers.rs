use super::*;
use crate::{
    application::{
        live_queue::LiveQueue, streaming_audio::StreamSegmentHandle, AudioPreviewPort,
        VoicePipelinePort,
    },
    domain::{
        InterpretOutcome, LiveSessionId, SegmentId, SegmentResult, StageDurations, VerbalixError,
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

pub(crate) struct FakePipeline {
    pub(crate) call_count: Arc<AtomicUsize>,
    pub(crate) should_fail: bool,
    pub(crate) delay_ms: u64,
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
        let count = Arc::clone(&self.call_count);
        let fail = self.should_fail;
        let delay = self.delay_ms;
        Box::pin(async move {
            count.fetch_add(1, Ordering::Relaxed);
            if delay > 0 {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
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

pub(crate) struct FakeStreamPipeline {
    pub(crate) call_count: Arc<AtomicUsize>,
    pub(crate) delay_ms: u64,
    pub(crate) prefill_samples: usize,
}

impl VoicePipelinePort for FakeStreamPipeline {
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
        let count = Arc::clone(&self.call_count);
        let delay = self.delay_ms;
        let prefill = self.prefill_samples;
        Box::pin(async move {
            count.fetch_add(1, Ordering::Relaxed);
            if delay > 0 {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            let buffer = Arc::new(Mutex::new(VecDeque::from(vec![0.0f32; prefill])));
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
                source_text: String::new(),
            })
        })
    }
}

pub(crate) struct FakePlayback {
    pub(crate) play_count: Arc<AtomicUsize>,
    pub(crate) delay_ms: u64,
}

impl AudioPreviewPort for FakePlayback {
    fn play(&self, _wav_bytes: Vec<u8>) -> Result<(), VerbalixError> {
        self.play_count.fetch_add(1, Ordering::Relaxed);
        if self.delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(self.delay_ms));
        }
        Ok(())
    }

    fn stop(&self) {}

    fn play_stream(&self, handle: StreamSegmentHandle) -> Result<(), VerbalixError> {
        self.play_count.fetch_add(1, Ordering::Relaxed);
        if self.delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(self.delay_ms));
        }
        while !handle.cancel.load(Ordering::Relaxed) && !handle.complete.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
}

pub(crate) fn always_accepts(
) -> Arc<dyn Fn(LiveSessionId, SegmentId) -> bool + Send + Sync + 'static> {
    Arc::new(|_, _| true)
}

pub(crate) fn accepts_only(
    sid: LiveSessionId,
) -> Arc<dyn Fn(LiveSessionId, SegmentId) -> bool + Send + Sync + 'static> {
    Arc::new(move |s: LiveSessionId, _| s == sid)
}

pub(crate) fn make_context() -> Arc<Mutex<crate::domain::TranslationContext>> {
    Arc::new(Mutex::new(crate::domain::TranslationContext::new()))
}

pub(crate) fn make_worker(
    pipeline: Arc<dyn VoicePipelinePort>,
    playback: Arc<dyn AudioPreviewPort>,
) -> (LiveWorker, Arc<Mutex<LiveQueue>>, Arc<AtomicUsize>) {
    let queue = Arc::new(Mutex::new(LiveQueue::new(8)));
    let ready_count = Arc::new(AtomicUsize::new(0));
    let ready_clone = Arc::clone(&ready_count);

    let worker = LiveWorker::new(
        pipeline,
        playback,
        Arc::clone(&queue),
        Arc::new(move |event| {
            if let WorkerEvent::Ready { .. } = event {
                ready_clone.fetch_add(1, Ordering::Relaxed);
            }
        }),
        always_accepts(),
        make_context(),
    );

    (worker, queue, ready_count)
}

pub(crate) fn session() -> LiveSessionId {
    LiveSessionId(uuid::Uuid::nil())
}

pub(crate) fn dispatch_cmd(session_id: LiveSessionId, segment_id: SegmentId) -> WorkerCommand {
    WorkerCommand::Dispatch {
        session_id,
        segment_id,
        wav_bytes: vec![],
        target_language: "en".to_owned(),
        token: "tok".to_owned(),
        t_capture_end: Instant::now(),
        context: vec![],
    }
}
