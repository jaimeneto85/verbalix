use crate::{
    application::{
        live_queue::LiveQueue,
        live_worker::{LiveWorker, WorkerCommand, WorkerEvent},
        AudioStreamPort,
    },
    domain::{
        EndpointEvent, Endpointer, EndpointerConfig, LiveSession, LiveState, SegmentId,
        StageDurations,
    },
    platform::audio_wav::{encode_wav, pcm_rms, resample_to_16k, TARGET_SAMPLE_RATE},
};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct LiveEventPayload {
    pub status: String,
    pub stage_ms: Option<StageDurations>,
    pub segment_id: Option<u64>,
    pub detected_language: Option<String>,
}

pub type LiveEventFn = Arc<dyn Fn(LiveEventPayload) + Send + Sync>;

pub(crate) const CIRCUIT_BREAKER_THRESHOLD: u32 = 3;

pub(crate) struct CoordinatorState {
    pub live_state: LiveState,
    pub session: Option<LiveSession>,
    pub circuit_failures: u32,
    pub last_segment_time: Option<Instant>,
}

pub(crate) struct AudioSinkState {
    pub endpointer: Endpointer,
    pub frame_buffer: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

impl AudioSinkState {
    pub fn new() -> Self {
        Self {
            endpointer: Endpointer::new(EndpointerConfig::default()),
            frame_buffer: Vec::new(),
            sample_rate: TARGET_SAMPLE_RATE,
            channels: 1,
        }
    }
}

type SinkFn = Box<dyn Fn(Vec<f32>, u32, u16) + Send + Sync + 'static>;

pub(crate) fn build_audio_sink(
    sink_state_arc: Arc<Mutex<AudioSinkState>>,
    state_arc: Arc<Mutex<CoordinatorState>>,
    worker_arc: Arc<Mutex<Option<LiveWorker>>>,
    token: String,
) -> SinkFn {
    Box::new(move |frames, sample_rate, channels| {
        let rms = pcm_rms(&frames);
        let event = {
            let mut ss = sink_state_arc.lock().unwrap();
            ss.sample_rate = sample_rate;
            ss.channels = channels;
            if ss.endpointer.is_open() {
                ss.frame_buffer.extend_from_slice(&frames);
            }
            ss.endpointer.push_frame(rms)
        };

        match event {
            Some(EndpointEvent::Opened) => {
                let mut ss = sink_state_arc.lock().unwrap();
                ss.frame_buffer.clear();
                ss.frame_buffer.extend_from_slice(&frames);
            }
            Some(EndpointEvent::Closed) | Some(EndpointEvent::MaxDurationReached) => {
                let (wav_bytes, session_id, segment_id, target_lang) = {
                    let mut ss = sink_state_arc.lock().unwrap();
                    let samples = resample_to_16k(&ss.frame_buffer, ss.sample_rate, ss.channels);
                    let wav = encode_wav(&samples, TARGET_SAMPLE_RATE);
                    ss.frame_buffer.clear();

                    let mut st = state_arc.lock().unwrap();
                    if let Some(ref mut session) = st.session {
                        let sid = session.id;
                        let seg = session.advance();
                        let lang = session.target_language.as_str().to_owned();
                        st.last_segment_time = Some(Instant::now());
                        (wav, sid, seg, lang)
                    } else {
                        return;
                    }
                };

                let guard = worker_arc.lock().unwrap();
                if let Some(ref w) = *guard {
                    w.dispatch(WorkerCommand::Dispatch {
                        session_id,
                        segment_id,
                        wav_bytes,
                        target_language: target_lang,
                        token: token.clone(),
                    });
                }
            }
            Some(EndpointEvent::Dropped) | None => {}
        }
    })
}

pub(crate) fn build_worker_callback(
    state_arc: Arc<Mutex<CoordinatorState>>,
    queue_arc: Arc<Mutex<LiveQueue>>,
    capture_arc: Arc<dyn AudioStreamPort>,
    on_event: LiveEventFn,
) -> Arc<dyn Fn(WorkerEvent) + Send + Sync + 'static> {
    Arc::new(move |event| match event {
        WorkerEvent::Failed {
            segment_id,
            session_id,
        } => {
            let current_session_matches = {
                let st = state_arc.lock().unwrap();
                st.session
                    .as_ref()
                    .map(|s| s.accepts(session_id, segment_id))
                    .unwrap_or(false)
            };
            if !current_session_matches {
                return;
            }
            let failed = {
                let mut st = state_arc.lock().unwrap();
                st.circuit_failures += 1;
                if st.circuit_failures < CIRCUIT_BREAKER_THRESHOLD {
                    st.live_state = LiveState::Recovering;
                }
                st.circuit_failures
            };
            if failed >= CIRCUIT_BREAKER_THRESHOLD {
                let mut st = state_arc.lock().unwrap();
                st.live_state = LiveState::Failed;
                st.session = None;
                capture_arc.stop_stream();
                queue_arc.lock().unwrap().reset(SegmentId(0));
                on_event(LiveEventPayload {
                    status: "error".to_owned(),
                    stage_ms: None,
                    segment_id: None,
                    detected_language: None,
                });
            }
        }
        WorkerEvent::Ready {
            segment_id,
            session_id,
            stage_ms,
            detected_language,
        } => {
            let current_session_matches = {
                let st = state_arc.lock().unwrap();
                st.session
                    .as_ref()
                    .map(|s| s.accepts(session_id, segment_id))
                    .unwrap_or(false)
            };
            if current_session_matches {
                let mut st = state_arc.lock().unwrap();
                st.circuit_failures = 0;
                if st.live_state == LiveState::Recovering {
                    st.live_state = LiveState::OnAir;
                }
            }
            on_event(LiveEventPayload {
                status: "speaking".to_owned(),
                stage_ms: Some(stage_ms),
                segment_id: Some(segment_id.0),
                detected_language: Some(detected_language),
            });
        }
        WorkerEvent::Dropped { segment_id } => {
            on_event(LiveEventPayload {
                status: "dropped".to_owned(),
                stage_ms: None,
                segment_id: Some(segment_id.0),
                detected_language: None,
            });
        }
    })
}
