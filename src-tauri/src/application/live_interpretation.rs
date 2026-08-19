use crate::{
    application::{
        live_queue::LiveQueue,
        live_session_setup::{
            build_audio_sink, build_worker_callback, AudioSinkState, CoordinatorState,
        },
        live_worker::LiveWorker,
        runtime_pause::{OnAirGuard, RuntimePause},
        AudioPreviewPort, AudioStreamPort, VirtualMicOutputPort, VoicePipelinePort,
    },
    domain::{LiveState, SegmentId, TranslationContext, VerbalixError},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

pub use crate::application::live_session_setup::{LiveEventFn, LiveEventPayload};

#[cfg(test)]
use crate::application::live_session_setup::CIRCUIT_BREAKER_THRESHOLD;

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
    virtual_mic: Arc<dyn VirtualMicOutputPort>,
    route: Arc<AtomicBool>,
    context: Arc<Mutex<TranslationContext>>,
}

impl LiveInterpretationCoordinator {
    pub fn new(
        pipeline: Arc<dyn VoicePipelinePort>,
        capture: Arc<dyn AudioStreamPort>,
        playback: Arc<dyn AudioPreviewPort>,
        pause: Arc<RuntimePause>,
        on_live_event: LiveEventFn,
        virtual_mic: Arc<dyn VirtualMicOutputPort>,
        route: Arc<AtomicBool>,
    ) -> Self {
        let queue = Arc::new(Mutex::new(LiveQueue::new(8)));
        Self {
            state: Arc::new(Mutex::new(CoordinatorState {
                live_state: LiveState::Idle,
                session: None,
                circuit_failures: 0,
                last_segment_time: None,
            })),
            sink_state: Arc::new(Mutex::new(AudioSinkState::new())),
            pipeline,
            capture,
            playback,
            pause,
            worker: Arc::new(Mutex::new(None)),
            queue,
            on_live_event,
            virtual_mic,
            route,
            context: Arc::new(Mutex::new(TranslationContext::new())),
        }
    }

    pub fn enter_live(
        &self,
        target_language: &str,
        _voice_profile_id: uuid::Uuid,
        token: String,
        route_to_virtual_mic: bool,
    ) -> Result<OnAirGuard, VerbalixError> {
        let lang = crate::domain::LanguageTag::parse(target_language)
            .ok_or(VerbalixError::TargetLanguageUnsupported)?;

        {
            let mut st = self.state.lock().unwrap();
            if st.live_state == LiveState::OnAir {
                return Err(VerbalixError::LiveSessionInactive);
            }
            st.live_state = LiveState::Preparing;
            st.session = Some(crate::domain::LiveSession::new(lang));
            st.circuit_failures = 0;
            st.last_segment_time = None;
        }

        self.context.lock().unwrap().reset();

        let routed = resolve_route(
            route_to_virtual_mic,
            self.virtual_mic.as_ref(),
            &self.on_live_event,
        );
        self.route.store(routed, Ordering::Relaxed);

        let sink = build_audio_sink(
            Arc::clone(&self.sink_state),
            Arc::clone(&self.state),
            Arc::clone(&self.worker),
            token,
            Arc::clone(&self.context),
        );

        let state_for_error = Arc::clone(&self.state);
        let error_sink: Box<dyn Fn() + Send + Sync + 'static> = Box::new(move || {
            let mut st = state_for_error.lock().unwrap();
            st.live_state = LiveState::Failed;
            st.session = None;
        });

        self.capture.start_stream(sink, error_sink).map_err(|_| {
            self.virtual_mic.close();
            self.route.store(false, Ordering::Relaxed);
            let mut st = self.state.lock().unwrap();
            st.live_state = LiveState::Idle;
            st.session = None;
            VerbalixError::AudioCaptureFailed
        })?;

        let state_for_accepts = Arc::clone(&self.state);
        let accepts_fn = Arc::new(
            move |sid: crate::domain::LiveSessionId, seg: crate::domain::SegmentId| {
                state_for_accepts
                    .lock()
                    .unwrap()
                    .session
                    .as_ref()
                    .map(|s| s.accepts(sid, seg))
                    .unwrap_or(false)
            },
        );

        let worker = LiveWorker::new(
            Arc::clone(&self.pipeline),
            Arc::clone(&self.playback),
            Arc::clone(&self.queue),
            build_worker_callback(
                Arc::clone(&self.state),
                Arc::clone(&self.queue),
                Arc::clone(&self.capture),
                Arc::clone(&self.on_live_event),
            ),
            accepts_fn,
            Arc::clone(&self.context),
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
            target_language: None,
            first_audio_ms: None,
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

        self.context.lock().unwrap().reset();

        self.route.store(false, Ordering::Relaxed);
        self.virtual_mic.close();

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

        crate::diagnostics::emit_latency_summary();

        self.on_live_event.as_ref()(LiveEventPayload {
            status: "idle".to_owned(),
            stage_ms: None,
            segment_id: None,
            detected_language: None,
            target_language: None,
            first_audio_ms: None,
        });
    }

    pub fn live_state(&self) -> LiveState {
        self.state.lock().unwrap().live_state.clone()
    }

    pub fn vmic_metrics(&self) -> crate::application::VirtualMicMetrics {
        self.virtual_mic.metrics()
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

    #[cfg(test)]
    pub(crate) fn context_snapshot(&self) -> Vec<String> {
        self.context.lock().unwrap().snapshot()
    }
}

fn resolve_route(
    route_to_virtual_mic: bool,
    virtual_mic: &dyn VirtualMicOutputPort,
    on_event: &LiveEventFn,
) -> bool {
    if !route_to_virtual_mic {
        return false;
    }
    match virtual_mic.open() {
        Ok(()) => true,
        Err(_) => {
            on_event(LiveEventPayload {
                status: "virtual-mic-fallback".to_owned(),
                stage_ms: None,
                segment_id: None,
                detected_language: None,
                target_language: None,
                first_audio_ms: None,
            });
            false
        }
    }
}

#[cfg(test)]
#[path = "live_interpretation_tests.rs"]
mod tests;
