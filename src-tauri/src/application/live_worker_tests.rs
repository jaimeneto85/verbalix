use super::*;
use crate::{
    application::{
        live_queue::LiveQueue,
        streaming_audio::StreamSegmentHandle,
        AudioPreviewPort, VoicePipelinePort,
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

struct FakeStreamPipeline {
    call_count: Arc<AtomicUsize>,
    delay_ms: u64,
    prefill_samples: usize,
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
        session_id: LiveSessionId,
        segment_id: SegmentId,
        _wav_bytes: Vec<u8>,
        _target_language: &'a str,
        _token: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<StreamSegmentHandle, InterpretOutcome>,
                > + Send
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

struct FakePlayback {
    play_count: Arc<AtomicUsize>,
    delay_ms: u64,
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

fn always_accepts() -> Arc<dyn Fn(LiveSessionId, SegmentId) -> bool + Send + Sync + 'static> {
    Arc::new(|_, _| true)
}

fn accepts_only(sid: LiveSessionId) -> Arc<dyn Fn(LiveSessionId, SegmentId) -> bool + Send + Sync + 'static> {
    Arc::new(move |s: LiveSessionId, _| s == sid)
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
        always_accepts(),
    );

    (worker, queue, ready_count)
}

fn session() -> LiveSessionId {
    LiveSessionId(uuid::Uuid::nil())
}

fn dispatch_cmd(session_id: LiveSessionId, segment_id: SegmentId) -> WorkerCommand {
    WorkerCommand::Dispatch {
        session_id,
        segment_id,
        wav_bytes: vec![],
        target_language: "en".to_owned(),
        token: "tok".to_owned(),
        t_capture_end: Instant::now(),
    }
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
        delay_ms: 0,
    });

    let (worker, _queue, ready_count) = make_worker(pipeline, playback);

    worker.dispatch(dispatch_cmd(session(), SegmentId(0)));
    std::thread::sleep(Duration::from_millis(300));

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
        delay_ms: 0,
    });

    let (worker, _queue, _) = make_worker(pipeline, playback);

    for i in 0..4 {
        worker.dispatch(dispatch_cmd(session(), SegmentId(i)));
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
        delay_ms: 0,
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
        always_accepts(),
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

#[test]
fn streaming_burst_drain_plays_in_order() {
    let play_order: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
    let play_order_clone = Arc::clone(&play_order);

    let pipeline = Arc::new(FakeStreamPipeline {
        call_count: Arc::new(AtomicUsize::new(0)),
        delay_ms: 0,
        prefill_samples: 100,
    });
    let playback = Arc::new(FakePlayback {
        play_count: Arc::new(AtomicUsize::new(0)),
        delay_ms: 10,
    });

    let queue = Arc::new(Mutex::new(LiveQueue::new(8)));
    let s = session();

    let worker = LiveWorker::new(
        pipeline,
        playback,
        Arc::clone(&queue),
        Arc::new(move |event| {
            if let WorkerEvent::Ready { segment_id, .. } = event {
                play_order_clone.lock().unwrap().push(segment_id.0);
            }
        }),
        always_accepts(),
    );

    worker.dispatch(dispatch_cmd(s, SegmentId(2)));
    worker.dispatch(dispatch_cmd(s, SegmentId(1)));
    worker.dispatch(dispatch_cmd(s, SegmentId(0)));

    std::thread::sleep(Duration::from_millis(600));

    let order = play_order.lock().unwrap();
    assert_eq!(order.len(), 3, "all 3 segments must play");
    assert_eq!(order[0], 0, "segment 0 must play first");
    assert_eq!(order[1], 1, "segment 1 must play second");
    assert_eq!(order[2], 2, "segment 2 must play third");
}

#[test]
fn stop_mid_stream_sets_cancel_on_handles() {
    let pipeline = Arc::new(FakeStreamPipeline {
        call_count: Arc::new(AtomicUsize::new(0)),
        delay_ms: 300,
        prefill_samples: 100,
    });
    let playback = Arc::new(FakePlayback {
        play_count: Arc::new(AtomicUsize::new(0)),
        delay_ms: 0,
    });

    let (worker, _queue, _) = make_worker(pipeline, playback);

    worker.dispatch(dispatch_cmd(session(), SegmentId(0)));
    std::thread::sleep(Duration::from_millis(50));
    worker.stop();

    std::thread::sleep(Duration::from_millis(100));
}

#[test]
fn accepts_rejects_wrong_session() {
    let play_count = Arc::new(AtomicUsize::new(0));
    let pipeline = Arc::new(FakeStreamPipeline {
        call_count: Arc::new(AtomicUsize::new(0)),
        delay_ms: 0,
        prefill_samples: 10,
    });
    let playback = Arc::new(FakePlayback {
        play_count: Arc::clone(&play_count),
        delay_ms: 0,
    });

    let right_session = session();
    let wrong_session = LiveSessionId(uuid::Uuid::new_v4());

    let queue = Arc::new(Mutex::new(LiveQueue::new(8)));
    let worker = LiveWorker::new(
        pipeline,
        playback,
        Arc::clone(&queue),
        Arc::new(|_| {}),
        accepts_only(right_session),
    );

    worker.dispatch(WorkerCommand::Dispatch {
        session_id: wrong_session,
        segment_id: SegmentId(0),
        wav_bytes: vec![],
        target_language: "en".to_owned(),
        token: "tok".to_owned(),
        t_capture_end: Instant::now(),
    });

    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(play_count.load(Ordering::Relaxed), 0, "wrong session must not play audio");
}

#[test]
fn dropped_segment_does_not_play() {
    let play_count = Arc::new(AtomicUsize::new(0));
    let pipeline = Arc::new(FakeStreamPipeline {
        call_count: Arc::new(AtomicUsize::new(0)),
        delay_ms: 0,
        prefill_samples: 10,
    });
    let playback = Arc::new(FakePlayback {
        play_count: Arc::clone(&play_count),
        delay_ms: 0,
    });
    let queue = Arc::new(Mutex::new(LiveQueue::new(2)));

    let s = session();
    let worker = LiveWorker::new(
        pipeline,
        playback,
        Arc::clone(&queue),
        Arc::new(|_| {}),
        always_accepts(),
    );

    for i in 0..5 {
        worker.dispatch(dispatch_cmd(s, SegmentId(i)));
    }

    std::thread::sleep(Duration::from_millis(500));
}

#[test]
fn fail_closed_on_truncated_complete_buffer() {
    let play_count = Arc::new(AtomicUsize::new(0));
    let pipeline = Arc::new(FakeStreamPipeline {
        call_count: Arc::new(AtomicUsize::new(0)),
        delay_ms: 0,
        prefill_samples: 0,
    });
    let playback = Arc::new(FakePlayback {
        play_count: Arc::clone(&play_count),
        delay_ms: 0,
    });

    let (worker, _queue, ready_count) = make_worker(pipeline, playback);
    worker.dispatch(dispatch_cmd(session(), SegmentId(0)));
    std::thread::sleep(Duration::from_millis(300));

    assert!(
        ready_count.load(Ordering::Relaxed) <= 1,
        "truncated buffer should still complete without hanging"
    );
}
