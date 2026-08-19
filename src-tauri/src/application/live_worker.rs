use crate::{
    application::{
        live_queue::LiveQueue, streaming_audio::StreamSegmentHandle, AudioPreviewPort,
        VoicePipelinePort,
    },
    diagnostics::{self, LatencyStage},
    domain::{
        InterpretOutcome, LiveSessionId, SegmentId, SegmentResult, StageDurations,
        TranslationContext,
    },
};
use std::{
    collections::HashMap,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::Mutex as AsyncMutex;

mod playback;
use playback::{call_interpret_stream, process_queue_events_async, WorkerCallbacks};

const MAX_IN_FLIGHT: usize = 2;

pub enum WorkerCommand {
    Dispatch {
        session_id: LiveSessionId,
        segment_id: SegmentId,
        wav_bytes: Vec<u8>,
        target_language: String,
        token: String,
        t_capture_end: Instant,
        context: Vec<String>,
    },
    Stop,
}

pub enum WorkerEvent {
    Ready {
        segment_id: SegmentId,
        session_id: LiveSessionId,
        stage_ms: StageDurations,
        detected_language: String,
        target_language: Option<String>,
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

impl LiveWorker {
    pub fn new(
        pipeline: Arc<dyn VoicePipelinePort>,
        playback: Arc<dyn AudioPreviewPort>,
        queue: Arc<Mutex<LiveQueue>>,
        on_event: Arc<dyn Fn(WorkerEvent) + Send + Sync>,
        accepts_fn: Arc<dyn Fn(LiveSessionId, SegmentId) -> bool + Send + Sync + 'static>,
        context: Arc<Mutex<TranslationContext>>,
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
                            let handles: Vec<_> = pending_handles.lock().unwrap().drain().collect();
                            for (_, h) in handles {
                                h.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
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
                            context: ctx_snapshot,
                        }) => {
                            let pipeline = Arc::clone(&pipeline);
                            let playback = Arc::clone(&playback);
                            let queue = Arc::clone(&queue);
                            let on_event = Arc::clone(&on_event);
                            let sem = Arc::clone(&sem);
                            let playback_lock = Arc::clone(&playback_lock);
                            let pending_handles = Arc::clone(&pending_handles);
                            let accepts_fn = Arc::clone(&accepts_fn);
                            let context = Arc::clone(&context);

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
                                    ctx_snapshot,
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
                                                detected_language: handle.detected_language.clone(),
                                                stage_ms: StageDurations {
                                                    stt: handle.stt_ms,
                                                    translate: handle.translate_ms,
                                                    tts: 0,
                                                },
                                            }),
                                        };
                                        pending_handles.lock().unwrap().insert(segment_id, handle);
                                        let events = queue.lock().unwrap().insert(outcome);
                                        process_queue_events_async(
                                            events,
                                            playback,
                                            playback_lock,
                                            pending_handles,
                                            WorkerCallbacks {
                                                accepts_fn,
                                                on_event,
                                                context,
                                            },
                                            t_capture_end,
                                        )
                                        .await;
                                    }
                                    Err(outcome) => {
                                        diagnostics::record_latency(
                                            LatencyStage::Ttfb,
                                            t_capture_end.elapsed().as_millis() as u64,
                                        );
                                        let events = queue.lock().unwrap().insert(outcome);
                                        process_queue_events_async(
                                            events,
                                            playback,
                                            playback_lock,
                                            pending_handles,
                                            WorkerCallbacks {
                                                accepts_fn,
                                                on_event,
                                                context,
                                            },
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
#[path = "live_worker_test_helpers.rs"]
mod test_helpers;

#[cfg(test)]
#[path = "live_worker_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "live_worker_streaming_tests.rs"]
mod streaming_tests;

#[cfg(test)]
#[path = "live_worker_context_tests.rs"]
mod context_tests;
