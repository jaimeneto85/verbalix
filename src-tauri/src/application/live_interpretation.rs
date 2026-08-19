use crate::{
    application::{
        live_queue::LiveQueue,
        live_worker::{LiveWorker, WorkerCommand, WorkerEvent},
        runtime_pause::{OnAirGuard, RuntimePause},
        AudioPreviewPort, AudioStreamPort, VoicePipelinePort,
    },
    domain::{
        EndpointEvent, Endpointer, EndpointerConfig, LanguageTag, LiveSession, LiveState,
        SegmentId, StageDurations, VerbalixError,
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

const CIRCUIT_BREAKER_THRESHOLD: u32 = 3;

struct CoordinatorState {
    live_state: LiveState,
    session: Option<LiveSession>,
    circuit_failures: u32,
    last_segment_time: Option<Instant>,
}

struct AudioSinkState {
    endpointer: Endpointer,
    frame_buffer: Vec<f32>,
    sample_rate: u32,
    channels: u16,
}

pub struct LiveInterpretationCoordinator {
    state: Arc<Mutex<CoordinatorState>>,
    sink_state: Arc<Mutex<AudioSinkState>>,
    pipeline: Arc<dyn VoicePipelinePort>,
    capture: Arc<dyn AudioStreamPort>,
    playback: Arc<dyn AudioPreviewPort>,
    pause: Arc<RuntimePause>,
    worker: Arc<Mutex<Option<LiveWorker>>>,
    queue: Arc<Mutex<LiveQueue>>,
    on_live_event: LiveEventFn,
}

impl LiveInterpretationCoordinator {
    pub fn new(
        pipeline: Arc<dyn VoicePipelinePort>,
        capture: Arc<dyn AudioStreamPort>,
        playback: Arc<dyn AudioPreviewPort>,
        pause: Arc<RuntimePause>,
        on_live_event: LiveEventFn,
    ) -> Self {
        let queue = Arc::new(Mutex::new(LiveQueue::new(8)));
        Self {
            state: Arc::new(Mutex::new(CoordinatorState {
                live_state: LiveState::Idle,
                session: None,
                circuit_failures: 0,
                last_segment_time: None,
            })),
            sink_state: Arc::new(Mutex::new(AudioSinkState {
                endpointer: Endpointer::new(EndpointerConfig::default()),
                frame_buffer: Vec::new(),
                sample_rate: TARGET_SAMPLE_RATE,
                channels: 1,
            })),
            pipeline,
            capture,
            playback,
            pause,
            worker: Arc::new(Mutex::new(None)),
            queue,
            on_live_event,
        }
    }

    pub fn enter_live(
        &self,
        target_language: &str,
        _voice_profile_id: uuid::Uuid,
        token: String,
    ) -> Result<OnAirGuard, VerbalixError> {
        let lang =
            LanguageTag::parse(target_language).ok_or(VerbalixError::TargetLanguageUnsupported)?;

        {
            let mut st = self.state.lock().unwrap();
            if st.live_state == LiveState::OnAir {
                return Err(VerbalixError::LiveSessionInactive);
            }
            st.live_state = LiveState::Preparing;
            st.session = Some(LiveSession::new(lang));
            st.circuit_failures = 0;
            st.last_segment_time = None;
        }

        let state_arc = Arc::clone(&self.state);
        let sink_state_arc = Arc::clone(&self.sink_state);
        let worker_arc = Arc::clone(&self.worker);
        let token_clone = token.clone();

        let sink: Box<dyn Fn(Vec<f32>, u32, u16) + Send + Sync + 'static> =
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
                            let samples =
                                resample_to_16k(&ss.frame_buffer, ss.sample_rate, ss.channels);
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
                                token: token_clone.clone(),
                            });
                        }
                    }
                    Some(EndpointEvent::Dropped) | None => {}
                }
            });

        let state_for_error = Arc::clone(&self.state);
        let error_sink: Box<dyn Fn() + Send + Sync + 'static> = Box::new(move || {
            let mut st = state_for_error.lock().unwrap();
            st.live_state = LiveState::Failed;
            st.session = None;
        });

        self.capture.start_stream(sink, error_sink).map_err(|_| {
            let mut st = self.state.lock().unwrap();
            st.live_state = LiveState::Idle;
            st.session = None;
            VerbalixError::AudioCaptureFailed
        })?;

        let state_for_worker = Arc::clone(&self.state);
        let queue_clone = Arc::clone(&self.queue);
        let capture_for_cb = Arc::clone(&self.capture);
        let on_event_for_worker = Arc::clone(&self.on_live_event);

        let worker = LiveWorker::new(
            Arc::clone(&self.pipeline),
            Arc::clone(&self.playback),
            Arc::clone(&self.queue),
            Arc::new(move |event| match event {
                WorkerEvent::Failed {
                    segment_id,
                    session_id,
                } => {
                    let current_session_matches = {
                        let st = state_for_worker.lock().unwrap();
                        st.session
                            .as_ref()
                            .map(|s| s.accepts(session_id, segment_id))
                            .unwrap_or(false)
                    };
                    if !current_session_matches {
                        return;
                    }
                    let failed = {
                        let mut st = state_for_worker.lock().unwrap();
                        st.circuit_failures += 1;
                        if st.circuit_failures < CIRCUIT_BREAKER_THRESHOLD {
                            st.live_state = LiveState::Recovering;
                        }
                        st.circuit_failures
                    };
                    if failed >= CIRCUIT_BREAKER_THRESHOLD {
                        let mut st = state_for_worker.lock().unwrap();
                        st.live_state = LiveState::Failed;
                        st.session = None;
                        capture_for_cb.stop_stream();
                        queue_clone.lock().unwrap().reset(SegmentId(0));
                        on_event_for_worker(LiveEventPayload {
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
                        let st = state_for_worker.lock().unwrap();
                        st.session
                            .as_ref()
                            .map(|s| s.accepts(session_id, segment_id))
                            .unwrap_or(false)
                    };
                    if current_session_matches {
                        let mut st = state_for_worker.lock().unwrap();
                        st.circuit_failures = 0;
                        if st.live_state == LiveState::Recovering {
                            st.live_state = LiveState::OnAir;
                        }
                    }
                    on_event_for_worker(LiveEventPayload {
                        status: "speaking".to_owned(),
                        stage_ms: Some(stage_ms),
                        segment_id: Some(segment_id.0),
                        detected_language: Some(detected_language),
                    });
                }
                WorkerEvent::Dropped { segment_id } => {
                    on_event_for_worker(LiveEventPayload {
                        status: "dropped".to_owned(),
                        stage_ms: None,
                        segment_id: Some(segment_id.0),
                        detected_language: None,
                    });
                }
            }),
        );

        *self.worker.lock().unwrap() = Some(worker);

        {
            let mut st = self.state.lock().unwrap();
            st.live_state = LiveState::OnAir;
        }

        self.on_live_event.as_ref()(LiveEventPayload {
            status: "listening".to_owned(),
            stage_ms: None,
            segment_id: None,
            detected_language: None,
        });

        Ok(self.pause.begin_on_air())
    }

    pub fn leave_live(&self) {
        let mut st = self.state.lock().unwrap();
        if matches!(st.live_state, LiveState::Idle | LiveState::Failed) {
            return;
        }
        st.live_state = LiveState::Stopping;
        st.session = None;
        drop(st);

        self.capture.stop_stream();
        self.playback.stop();

        if let Some(w) = self.worker.lock().unwrap().take() {
            w.stop();
        }

        self.queue.lock().unwrap().reset(SegmentId(0));
        self.sink_state.lock().unwrap().endpointer.reset();

        let mut st = self.state.lock().unwrap();
        st.live_state = LiveState::Idle;
        drop(st);

        self.on_live_event.as_ref()(LiveEventPayload {
            status: "idle".to_owned(),
            stage_ms: None,
            segment_id: None,
            detected_language: None,
        });
    }

    pub fn live_state(&self) -> LiveState {
        self.state.lock().unwrap().live_state.clone()
    }

    #[cfg(test)]
    pub(crate) fn simulate_segment_failure(&self) {
        let failed = {
            let mut st = self.state.lock().unwrap();
            st.circuit_failures += 1;
            if st.circuit_failures < CIRCUIT_BREAKER_THRESHOLD {
                st.live_state = LiveState::Recovering;
            }
            st.circuit_failures
        };
        if failed >= CIRCUIT_BREAKER_THRESHOLD {
            let mut st = self.state.lock().unwrap();
            st.live_state = LiveState::Failed;
            st.session = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn active_session_id(&self) -> Option<crate::domain::LiveSessionId> {
        self.state.lock().unwrap().session.as_ref().map(|s| s.id)
    }
}

#[cfg(test)]
#[path = "live_interpretation_tests.rs"]
mod tests;
