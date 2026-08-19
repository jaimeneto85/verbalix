use crate::{
    application::{
        live_queue::{LiveQueue, QueueEvent},
        streaming_audio::StreamSegmentHandle,
        AudioPreviewPort, VoicePipelinePort,
    },
    diagnostics::{self, LatencyStage},
    domain::{InterpretOutcome, LiveSessionId, SegmentId, SegmentResult, StageDurations},
};
use std::{
    collections::HashMap,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::Mutex as AsyncMutex;

const MAX_IN_FLIGHT: usize = 2;

pub enum WorkerCommand {
    Dispatch {
        session_id: LiveSessionId,
        segment_id: SegmentId,
        wav_bytes: Vec<u8>,
        target_language: String,
        token: String,
        t_capture_end: Instant,
    },
    Stop,
}

pub enum WorkerEvent {
    Ready {
        segment_id: SegmentId,
        session_id: LiveSessionId,
        stage_ms: StageDurations,
        detected_language: String,
        first_audio_ms: Option<u64>,
    },
    Dropped {
        segment_id: SegmentId,
    },
    Failed {
        segment_id: SegmentId,
        session_id: LiveSessionId,
    },
}

pub struct LiveWorker {
    cmd_tx: mpsc::SyncSender<WorkerCommand>,
}

async fn call_interpret_stream(
    pipeline: Arc<dyn VoicePipelinePort>,
    session_id: LiveSessionId,
    segment_id: SegmentId,
    wav_bytes: Vec<u8>,
    target_language: String,
    token: String,
) -> Result<StreamSegmentHandle, InterpretOutcome> {
    pipeline
        .interpret_stream(session_id, segment_id, wav_bytes, &target_language, &token)
        .await
}

async fn process_queue_events_async(
    events: Vec<QueueEvent>,
    playback: Arc<dyn AudioPreviewPort>,
    playback_lock: Arc<AsyncMutex<()>>,
    pending_handles: Arc<Mutex<HashMap<SegmentId, StreamSegmentHandle>>>,
    accepts_fn: Arc<dyn Fn(LiveSessionId, SegmentId) -> bool + Send + Sync>,
    on_event: Arc<dyn Fn(WorkerEvent) + Send + Sync>,
    t_capture_end: Instant,
) {
    for event in events {
        match event {
            QueueEvent::Ready(out) => {
                let emit_seg = out.segment_id;
                let emit_session = out.session_id;

                if !accepts_fn(emit_session, emit_seg) {
                    if let Some(h) = pending_handles.lock().unwrap().remove(&emit_seg) {
                        h.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    on_event(WorkerEvent::Failed {
                        segment_id: emit_seg,
                        session_id: emit_session,
                    });
                    continue;
                }

                if let Ok(ref result) = out.result {
                    let handle_opt = pending_handles.lock().unwrap().remove(&emit_seg);

                    let _guard = playback_lock.lock().await;
                    let first_audio_ms = t_capture_end.elapsed().as_millis() as u64;
                    diagnostics::record_latency(LatencyStage::FirstAudio, first_audio_ms);

                    if let Some(h) = handle_opt {
                        let pb = Arc::clone(&playback);
                        let _ =
                            tokio::task::spawn_blocking(move || pb.play_stream(h)).await;
                    } else if !result.audio_base64.is_empty() {
                        use base64::{engine::general_purpose::STANDARD, Engine};
                        if let Ok(wav) = STANDARD.decode(&result.audio_base64) {
                            let pb = Arc::clone(&playback);
                            let _ =
                                tokio::task::spawn_blocking(move || pb.play(wav)).await;
                        }
                    }

                    diagnostics::record_latency(
                        LatencyStage::PlaybackEnd,
                        t_capture_end.elapsed().as_millis() as u64,
                    );
                    drop(_guard);

                    on_event(WorkerEvent::Ready {
                        segment_id: emit_seg,
                        session_id: emit_session,
                        stage_ms: result.stage_ms.clone(),
                        detected_language: result.detected_language.clone(),
                        first_audio_ms: Some(first_audio_ms),
                    });
                } else {
                    if let Some(h) = pending_handles.lock().unwrap().remove(&emit_seg) {
                        h.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    on_event(WorkerEvent::Failed {
                        segment_id: emit_seg,
                        session_id: emit_session,
                    });
                }
            }
            QueueEvent::Dropped { segment_id } => {
                if let Some(h) = pending_handles.lock().unwrap().remove(&segment_id) {
                    h.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                on_event(WorkerEvent::Dropped { segment_id });
            }
        }
    }
}

impl LiveWorker {
    pub fn new(
        pipeline: Arc<dyn VoicePipelinePort>,
        playback: Arc<dyn AudioPreviewPort>,
        queue: Arc<Mutex<LiveQueue>>,
        on_event: Arc<dyn Fn(WorkerEvent) + Send + Sync>,
        accepts_fn: Arc<dyn Fn(LiveSessionId, SegmentId) -> bool + Send + Sync + 'static>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::sync_channel::<WorkerCommand>(32);

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("worker runtime");

            let sem = Arc::new(tokio::sync::Semaphore::new(MAX_IN_FLIGHT));
            let playback_lock = Arc::new(AsyncMutex::new(()));
            let pending_handles: Arc<Mutex<HashMap<SegmentId, StreamSegmentHandle>>> =
                Arc::new(Mutex::new(HashMap::new()));

            rt.block_on(async move {
                loop {
                    match cmd_rx.try_recv() {
                        Ok(WorkerCommand::Stop) => {
                            let handles: Vec<_> =
                                pending_handles.lock().unwrap().drain().collect();
                            for (_, h) in handles {
                                h.cancel
                                    .store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                            let drained = queue.lock().unwrap().drain_all();
                            for o in drained {
                                on_event(WorkerEvent::Dropped {
                                    segment_id: o.segment_id,
                                });
                            }
                            break;
                        }
                        Ok(WorkerCommand::Dispatch {
                            session_id,
                            segment_id,
                            wav_bytes,
                            target_language,
                            token,
                            t_capture_end,
                        }) => {
                            let pipeline = Arc::clone(&pipeline);
                            let playback = Arc::clone(&playback);
                            let queue = Arc::clone(&queue);
                            let on_event = Arc::clone(&on_event);
                            let sem = Arc::clone(&sem);
                            let playback_lock = Arc::clone(&playback_lock);
                            let pending_handles = Arc::clone(&pending_handles);
                            let accepts_fn = Arc::clone(&accepts_fn);

                            tokio::task::spawn(async move {
                                let _permit = sem.acquire_owned().await.unwrap();

                                diagnostics::record_latency(
                                    LatencyStage::CaptureToRequest,
                                    t_capture_end.elapsed().as_millis() as u64,
                                );

                                match call_interpret_stream(
                                    pipeline,
                                    session_id,
                                    segment_id,
                                    wav_bytes,
                                    target_language,
                                    token,
                                )
                                .await
                                {
                                    Ok(handle) => {
                                        diagnostics::record_latency(
                                            LatencyStage::Ttfb,
                                            t_capture_end.elapsed().as_millis() as u64,
                                        );
                                        let outcome = InterpretOutcome {
                                            session_id,
                                            segment_id,
                                            result: Ok(SegmentResult {
                                                audio_base64: String::new(),
                                                detected_language: handle
                                                    .detected_language
                                                    .clone(),
                                                stage_ms: StageDurations {
                                                    stt: handle.stt_ms,
                                                    translate: handle.translate_ms,
                                                    tts: 0,
                                                },
                                            }),
                                        };
                                        pending_handles
                                            .lock()
                                            .unwrap()
                                            .insert(segment_id, handle);
                                        let events =
                                            queue.lock().unwrap().insert(outcome);
                                        process_queue_events_async(
                                            events,
                                            playback,
                                            playback_lock,
                                            pending_handles,
                                            accepts_fn,
                                            on_event,
                                            t_capture_end,
                                        )
                                        .await;
                                    }
                                    Err(outcome) => {
                                        diagnostics::record_latency(
                                            LatencyStage::Ttfb,
                                            t_capture_end.elapsed().as_millis() as u64,
                                        );
                                        let events =
                                            queue.lock().unwrap().insert(outcome);
                                        process_queue_events_async(
                                            events,
                                            playback,
                                            playback_lock,
                                            pending_handles,
                                            accepts_fn,
                                            on_event,
                                            t_capture_end,
                                        )
                                        .await;
                                    }
                                }
                            });
                        }
                        Err(mpsc::TryRecvError::Empty) => {
                            tokio::time::sleep(Duration::from_millis(5)).await;
                        }
                        Err(mpsc::TryRecvError::Disconnected) => break,
                    }
                }
            });
        });

        Self { cmd_tx }
    }

    pub fn dispatch(&self, cmd: WorkerCommand) {
        let _ = self.cmd_tx.try_send(cmd);
    }

    pub fn stop(&self) {
        let _ = self.cmd_tx.try_send(WorkerCommand::Stop);
    }
}

#[cfg(test)]
#[path = "live_worker_tests.rs"]
mod tests;
