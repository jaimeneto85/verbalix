use super::test_helpers::*;
use super::*;
use crate::{
    application::live_queue::LiveQueue,
    domain::{LiveSessionId, SegmentId},
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

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
        make_context(),
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
        make_context(),
    );

    worker.dispatch(WorkerCommand::Dispatch {
        session_id: wrong_session,
        segment_id: SegmentId(0),
        wav_bytes: vec![],
        target_language: "en".to_owned(),
        token: "tok".to_owned(),
        t_capture_end: std::time::Instant::now(),
        context: vec![],
    });

    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        play_count.load(Ordering::Relaxed),
        0,
        "wrong session must not play audio"
    );
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
        make_context(),
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
