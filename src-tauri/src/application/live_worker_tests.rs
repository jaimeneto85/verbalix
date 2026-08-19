use super::*;
use crate::{
    application::{live_queue::LiveQueue, AudioPreviewPort, VoicePipelinePort},
    domain::{
        InterpretOutcome, LiveSessionId, SegmentId, SegmentResult, StageDurations, VerbalixError,
    },
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

struct FakePipeline {
    call_count: Arc<AtomicUsize>,
    should_fail: bool,
    delay_ms: u64,
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

struct FakePlayback {
    play_count: Arc<AtomicUsize>,
}

impl AudioPreviewPort for FakePlayback {
    fn play(&self, _wav_bytes: Vec<u8>) -> Result<(), VerbalixError> {
        self.play_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn stop(&self) {}
}

fn make_worker(
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
    );

    (worker, queue, ready_count)
}

fn session() -> LiveSessionId {
    LiveSessionId(uuid::Uuid::nil())
}

#[test]
fn dispatch_calls_pipeline_and_plays_audio() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let play_count = Arc::new(AtomicUsize::new(0));

    let pipeline = Arc::new(FakePipeline {
        call_count: Arc::clone(&call_count),
        should_fail: false,
        delay_ms: 0,
    });
    let playback = Arc::new(FakePlayback {
        play_count: Arc::clone(&play_count),
    });

    let (worker, _queue, ready_count) = make_worker(pipeline, playback);

    worker.dispatch(WorkerCommand::Dispatch {
        session_id: session(),
        segment_id: SegmentId(0),
        wav_bytes: vec![],
        target_language: "en".to_owned(),
        token: "tok".to_owned(),
    });

    std::thread::sleep(Duration::from_millis(200));

    assert!(call_count.load(Ordering::Relaxed) >= 1);
    assert!(ready_count.load(Ordering::Relaxed) >= 1);
}

#[test]
fn concurrent_cap_limits_to_two_in_flight() {
    let call_count = Arc::new(AtomicUsize::new(0));

    let pipeline = Arc::new(FakePipeline {
        call_count: Arc::clone(&call_count),
        should_fail: false,
        delay_ms: 100,
    });
    let playback = Arc::new(FakePlayback {
        play_count: Arc::new(AtomicUsize::new(0)),
    });

    let (worker, _queue, _) = make_worker(pipeline, playback);

    for i in 0..4 {
        worker.dispatch(WorkerCommand::Dispatch {
            session_id: session(),
            segment_id: SegmentId(i),
            wav_bytes: vec![],
            target_language: "en".to_owned(),
            token: "tok".to_owned(),
        });
    }

    std::thread::sleep(Duration::from_millis(80));

    assert!(call_count.load(Ordering::Relaxed) <= 2);
}

#[test]
fn stop_drains_queue() {
    let pipeline = Arc::new(FakePipeline {
        call_count: Arc::new(AtomicUsize::new(0)),
        should_fail: false,
        delay_ms: 500,
    });
    let playback = Arc::new(FakePlayback {
        play_count: Arc::new(AtomicUsize::new(0)),
    });

    let queue = Arc::new(Mutex::new(LiveQueue::new(8)));
    let drop_count = Arc::new(AtomicUsize::new(0));
    let drop_clone = Arc::clone(&drop_count);

    let worker = LiveWorker::new(
        pipeline,
        playback,
        Arc::clone(&queue),
        Arc::new(move |event| {
            if let WorkerEvent::Dropped { .. } = event {
                drop_clone.fetch_add(1, Ordering::Relaxed);
            }
        }),
    );

    {
        let s = session();
        let pending = InterpretOutcome {
            session_id: s,
            segment_id: SegmentId(5),
            result: Ok(SegmentResult {
                audio_base64: String::new(),
                detected_language: "en".to_owned(),
                stage_ms: StageDurations {
                    stt: 0,
                    translate: 0,
                    tts: 0,
                },
            }),
        };
        queue.lock().unwrap().insert(pending);
    }

    worker.stop();
    std::thread::sleep(Duration::from_millis(100));

    assert!(drop_count.load(Ordering::Relaxed) >= 1);
}
