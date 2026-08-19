use crate::{
    application::{
        live_queue::LiveQueue,
        live_worker::{LiveWorker, WorkerCommand, WorkerEvent},
        runtime_pause::{OnAirGuard, RuntimePause},
        AudioPreviewPort, AudioStreamPort, VoicePipelinePort,
    },
    domain::{
        EndpointEvent, Endpointer, EndpointerConfig, LanguageTag, LiveSession,
        LiveState, SegmentId, VerbalixError,
    },
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CIRCUIT_BREAKER_THRESHOLD: u32 = 3;
const IDLE_TIMEOUT: Duration = Duration::from_secs(120);
const TARGET_SAMPLE_RATE: u32 = 16_000;

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
}

fn pcm_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}

fn resample_to_16k(samples: &[f32], src_rate: u32, channels: u16) -> Vec<i16> {
    let ch = channels.max(1) as usize;
    let mono: Vec<f32> = samples
        .chunks(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect();

    if src_rate == TARGET_SAMPLE_RATE {
        return mono
            .iter()
            .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
            .collect();
    }

    let ratio = src_rate as f64 / TARGET_SAMPLE_RATE as f64;
    let out_len = (mono.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos as usize;
        let frac = (pos - idx as f64) as f32;
        let a = mono.get(idx).copied().unwrap_or(0.0);
        let b = mono.get(idx + 1).copied().unwrap_or(a);
        let sample = a + (b - a) * frac;
        output.push((sample * 32767.0).clamp(-32768.0, 32767.0) as i16);
    }
    output
}

fn encode_wav(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let data_size = (samples.len() * 2) as u32;
    let file_size = data_size + 36;
    let mut buf = Vec::with_capacity((data_size + 44) as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    for s in samples {
        buf.extend_from_slice(&s.to_le_bytes());
    }
    buf
}

impl LiveInterpretationCoordinator {
    pub fn new(
        pipeline: Arc<dyn VoicePipelinePort>,
        capture: Arc<dyn AudioStreamPort>,
        playback: Arc<dyn AudioPreviewPort>,
        pause: Arc<RuntimePause>,
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
        }
    }

    pub fn enter_live(
        &self,
        target_language: &str,
        _voice_profile_id: uuid::Uuid,
        token: String,
    ) -> Result<OnAirGuard, VerbalixError> {
        let lang = LanguageTag::parse(target_language)
            .ok_or(VerbalixError::TargetLanguageUnsupported)?;

        {
            let mut st = self.state.lock().unwrap();
            if st.live_state == LiveState::OnAir {
                return Err(VerbalixError::LiveSessionInactive);
            }
            st.live_state = LiveState::Preparing;
            st.session = Some(LiveSession::new(lang.clone()));
            st.circuit_failures = 0;
            st.last_segment_time = None;
        }

        let state_arc = Arc::clone(&self.state);
        let sink_state_arc = Arc::clone(&self.sink_state);
        let worker_arc = Arc::clone(&self.worker);
        let token_clone = token.clone();
        let lang_str = lang.as_str().to_owned();

        let sink: Box<dyn Fn(Vec<f32>, u32, u16) + Send + 'static> = Box::new(
            move |frames, sample_rate, channels| {
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
                        let (wav_bytes, session_id, segment_id) = {
                            let mut ss = sink_state_arc.lock().unwrap();
                            let samples =
                                resample_to_16k(&ss.frame_buffer, ss.sample_rate, ss.channels);
                            let wav = encode_wav(&samples, TARGET_SAMPLE_RATE);
                            ss.frame_buffer.clear();

                            let mut st = state_arc.lock().unwrap();
                            if let Some(ref mut session) = st.session {
                                let sid = session.id;
                                let seg = session.advance();
                                st.last_segment_time = Some(Instant::now());
                                (wav, sid, seg)
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
                                target_language: lang_str.clone(),
                                token: token_clone.clone(),
                            });
                        }
                    }
                    Some(EndpointEvent::Dropped) | None => {}
                }
            },
        );

        let state_for_error = Arc::clone(&self.state);
        let error_sink: Box<dyn Fn() + Send + 'static> = Box::new(move || {
            let mut st = state_for_error.lock().unwrap();
            st.live_state = LiveState::Failed;
            st.session = None;
        });

        self.capture
            .start_stream(sink, error_sink)
            .map_err(|_| {
                let mut st = self.state.lock().unwrap();
                st.live_state = LiveState::Idle;
                st.session = None;
                VerbalixError::AudioCaptureFailed
            })?;

        let state_for_worker = Arc::clone(&self.state);
        let queue_clone = Arc::clone(&self.queue);
        let capture_for_cb = Arc::clone(&self.capture);

        let worker = LiveWorker::new(
            Arc::clone(&self.pipeline),
            Arc::clone(&self.playback),
            Arc::clone(&self.queue),
            Arc::new(move |event| match event {
                WorkerEvent::SegmentFailed { .. } => {
                    let failed = {
                        let mut st = state_for_worker.lock().unwrap();
                        st.circuit_failures += 1;
                        st.circuit_failures
                    };
                    if failed >= CIRCUIT_BREAKER_THRESHOLD {
                        let mut st = state_for_worker.lock().unwrap();
                        st.live_state = LiveState::Failed;
                        st.session = None;
                        capture_for_cb.stop_stream();
                        queue_clone.lock().unwrap().reset(SegmentId(0));
                    }
                }
                WorkerEvent::SegmentReady { .. } => {
                    let mut st = state_for_worker.lock().unwrap();
                    st.circuit_failures = 0;
                }
                WorkerEvent::SegmentDropped { .. } => {}
            }),
        );

        *self.worker.lock().unwrap() = Some(worker);

        {
            let mut st = self.state.lock().unwrap();
            st.live_state = LiveState::OnAir;
        }

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

        if let Some(w) = self.worker.lock().unwrap().take() {
            w.stop();
        }

        self.queue.lock().unwrap().reset(SegmentId(0));
        self.sink_state.lock().unwrap().endpointer.reset();

        let mut st = self.state.lock().unwrap();
        st.live_state = LiveState::Idle;
    }

    pub fn live_state(&self) -> LiveState {
        self.state.lock().unwrap().live_state.clone()
    }

    #[cfg(test)]
    pub(crate) fn simulate_segment_failure(&self) {
        let failed = {
            let mut st = self.state.lock().unwrap();
            st.circuit_failures += 1;
            st.circuit_failures
        };
        if failed >= CIRCUIT_BREAKER_THRESHOLD {
            let mut st = self.state.lock().unwrap();
            st.live_state = LiveState::Failed;
            st.session = None;
        }
    }

    #[cfg(test)]
    pub(crate) fn active_session_id(&self) -> Option<LiveSessionId> {
        self.state
            .lock()
            .unwrap()
            .session
            .as_ref()
            .map(|s| s.id)
    }
}

#[cfg(test)]
#[path = "live_interpretation_tests.rs"]
mod tests;
