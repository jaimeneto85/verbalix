use super::test_helpers::*;
use super::*;
use crate::{
    application::live_queue::LiveQueue,
    domain::{InterpretOutcome, SegmentId, SegmentResult, StageDurations},
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

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
        make_context(),
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
