use crate::{
    application::{
        live_queue::QueueEvent, streaming_audio::StreamSegmentHandle, AudioPreviewPort,
        VoicePipelinePort,
    },
    diagnostics::{self, LatencyStage},
    domain::{InterpretOutcome, LiveSessionId, SegmentId, TranslationContext},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::sync::Mutex as AsyncMutex;

use super::WorkerEvent;

pub(super) struct WorkerCallbacks {
    pub(super) accepts_fn: Arc<dyn Fn(LiveSessionId, SegmentId) -> bool + Send + Sync>,
    pub(super) on_event: Arc<dyn Fn(WorkerEvent) + Send + Sync>,
    pub(super) context: Arc<Mutex<TranslationContext>>,
}

pub(super) async fn call_interpret_stream(
    pipeline: Arc<dyn VoicePipelinePort>,
    session_id: LiveSessionId,
    segment_id: SegmentId,
    wav_bytes: Vec<u8>,
    target_language: String,
    token: String,
    context: Vec<String>,
) -> Result<StreamSegmentHandle, InterpretOutcome> {
    pipeline
        .interpret_stream(
            session_id,
            segment_id,
            wav_bytes,
            &target_language,
            &token,
            context,
        )
        .await
}

pub(super) async fn process_queue_events_async(
    events: Vec<QueueEvent>,
    playback: Arc<dyn AudioPreviewPort>,
    playback_lock: Arc<AsyncMutex<()>>,
    pending_handles: Arc<Mutex<HashMap<SegmentId, StreamSegmentHandle>>>,
    cb: WorkerCallbacks,
    t_capture_end: Instant,
) {
    for event in events {
        match event {
            QueueEvent::Ready(out) => {
                let emit_seg = out.segment_id;
                let emit_session = out.session_id;

                if !(cb.accepts_fn)(emit_session, emit_seg) {
                    if let Some(h) = pending_handles.lock().unwrap().remove(&emit_seg) {
                        h.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    (cb.on_event)(WorkerEvent::Failed {
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

                    let handle_target = if let Some(h) = handle_opt {
                        let source = h.source_text.clone();
                        let target = h.target_language.clone();
                        let pb = Arc::clone(&playback);
                        let play_ok = matches!(
                            tokio::task::spawn_blocking(move || pb.play_stream(h)).await,
                            Ok(Ok(()))
                        );
                        if play_ok && (cb.accepts_fn)(emit_session, emit_seg) {
                            cb.context.lock().unwrap().push(&source);
                        }
                        Some(target)
                    } else {
                        if !result.audio_base64.is_empty() {
                            use base64::{engine::general_purpose::STANDARD, Engine};
                            if let Ok(wav) = STANDARD.decode(&result.audio_base64) {
                                let pb = Arc::clone(&playback);
                                let _ = tokio::task::spawn_blocking(move || pb.play(wav)).await;
                            }
                        }
                        None
                    };

                    diagnostics::record_latency(
                        LatencyStage::PlaybackEnd,
                        t_capture_end.elapsed().as_millis() as u64,
                    );
                    drop(_guard);

                    (cb.on_event)(WorkerEvent::Ready {
                        segment_id: emit_seg,
                        session_id: emit_session,
                        stage_ms: result.stage_ms.clone(),
                        detected_language: result.detected_language.clone(),
                        target_language: handle_target,
                        first_audio_ms: Some(first_audio_ms),
                    });
                } else {
                    if let Some(h) = pending_handles.lock().unwrap().remove(&emit_seg) {
                        h.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    (cb.on_event)(WorkerEvent::Failed {
                        segment_id: emit_seg,
                        session_id: emit_session,
                    });
                }
            }
            QueueEvent::Dropped { segment_id } => {
                if let Some(h) = pending_handles.lock().unwrap().remove(&segment_id) {
                    h.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                (cb.on_event)(WorkerEvent::Dropped { segment_id });
            }
        }
    }
}
