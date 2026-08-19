use base64::{engine::general_purpose::STANDARD, Engine};
use crate::{
    application::{
        live_queue::{LiveQueue, QueueEvent},
        AudioPreviewPort, VoicePipelinePort,
    },
    domain::{InterpretOutcome, LiveSessionId, SegmentId, StageDurations},
};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tokio::sync::Semaphore;

const MAX_IN_FLIGHT: usize = 2;

pub enum WorkerCommand {
    Dispatch {
        session_id: LiveSessionId,
        segment_id: SegmentId,
        wav_bytes: Vec<u8>,
        target_language: String,
        token: String,
    },
    Stop,
}

pub enum WorkerEvent {
    Ready {
        segment_id: SegmentId,
        session_id: LiveSessionId,
        stage_ms: StageDurations,
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

async fn call_interpret(
    pipeline: Arc<dyn VoicePipelinePort>,
    session_id: LiveSessionId,
    segment_id: SegmentId,
    wav_bytes: Vec<u8>,
    target_language: String,
    token: String,
) -> InterpretOutcome {
    pipeline
        .interpret(session_id, segment_id, wav_bytes, &target_language, &token)
        .await
}

fn process_queue_events(
    events: Vec<QueueEvent>,
    playback: &dyn AudioPreviewPort,
    on_event: &(dyn Fn(WorkerEvent) + Send + Sync),
) {
    for event in events {
        match event {
            QueueEvent::Ready(out) => {
                let emit_seg = out.segment_id;
                let emit_session = out.session_id;
                if let Ok(ref result) = out.result {
                    if let Ok(wav) = STANDARD.decode(&result.audio_base64) {
                        let _ = playback.play(wav);
                    }
                    on_event(WorkerEvent::Ready {
                        segment_id: emit_seg,
                        session_id: emit_session,
                        stage_ms: result.stage_ms.clone(),
                    });
                } else {
                    on_event(WorkerEvent::Failed {
                        segment_id: emit_seg,
                        session_id: emit_session,
                    });
                }
            }
            QueueEvent::Dropped { segment_id } => {
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
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::sync_channel::<WorkerCommand>(32);

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("worker runtime");

            let sem = Arc::new(Semaphore::new(MAX_IN_FLIGHT));

            rt.block_on(async move {
                loop {
                    match cmd_rx.try_recv() {
                        Ok(WorkerCommand::Stop) => {
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
                        }) => {
                            let pipeline = Arc::clone(&pipeline);
                            let playback = Arc::clone(&playback);
                            let queue = Arc::clone(&queue);
                            let on_event = Arc::clone(&on_event);
                            let sem = Arc::clone(&sem);

                            tokio::task::spawn(async move {
                                let _permit = sem.acquire_owned().await.unwrap();

                                let outcome = call_interpret(
                                    pipeline,
                                    session_id,
                                    segment_id,
                                    wav_bytes,
                                    target_language,
                                    token,
                                )
                                .await;

                                let events = queue.lock().unwrap().insert(outcome);
                                process_queue_events(events, playback.as_ref(), on_event.as_ref());
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
