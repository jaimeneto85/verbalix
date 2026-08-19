use std::sync::{Mutex, OnceLock};

const BUCKET_BOUNDARIES_MS: [u64; 10] = [50, 100, 150, 200, 300, 500, 750, 1000, 2000, 5000];
const N_LATENCY_BUCKETS: usize = BUCKET_BOUNDARIES_MS.len() + 1;

#[derive(Copy, Clone)]
pub(crate) enum LatencyStage {
    CaptureToRequest,
    Ttfb,
    FirstAudio,
    PlaybackEnd,
}

pub(crate) struct LiveLatencyAggregator {
    capture_to_request: [u64; N_LATENCY_BUCKETS],
    ttfb: [u64; N_LATENCY_BUCKETS],
    first_audio: [u64; N_LATENCY_BUCKETS],
    playback_end: [u64; N_LATENCY_BUCKETS],
    pub(crate) underruns: u64,
}

impl LiveLatencyAggregator {
    pub(crate) const fn new() -> Self {
        Self {
            capture_to_request: [0; N_LATENCY_BUCKETS],
            ttfb: [0; N_LATENCY_BUCKETS],
            first_audio: [0; N_LATENCY_BUCKETS],
            playback_end: [0; N_LATENCY_BUCKETS],
            underruns: 0,
        }
    }

    pub(crate) fn record(&mut self, stage: LatencyStage, ms: u64) {
        let idx = BUCKET_BOUNDARIES_MS
            .iter()
            .position(|&b| ms <= b)
            .unwrap_or(N_LATENCY_BUCKETS - 1);
        self.buckets_mut(stage)[idx] += 1;
    }

    pub(crate) fn p50(&self, stage: LatencyStage) -> u64 {
        percentile_from_buckets(self.buckets(stage), 50)
    }

    pub(crate) fn p95(&self, stage: LatencyStage) -> u64 {
        percentile_from_buckets(self.buckets(stage), 95)
    }

    pub(crate) fn increment_underruns(&mut self) {
        self.underruns += 1;
    }

    fn buckets(&self, stage: LatencyStage) -> &[u64; N_LATENCY_BUCKETS] {
        match stage {
            LatencyStage::CaptureToRequest => &self.capture_to_request,
            LatencyStage::Ttfb => &self.ttfb,
            LatencyStage::FirstAudio => &self.first_audio,
            LatencyStage::PlaybackEnd => &self.playback_end,
        }
    }

    fn buckets_mut(&mut self, stage: LatencyStage) -> &mut [u64; N_LATENCY_BUCKETS] {
        match stage {
            LatencyStage::CaptureToRequest => &mut self.capture_to_request,
            LatencyStage::Ttfb => &mut self.ttfb,
            LatencyStage::FirstAudio => &mut self.first_audio,
            LatencyStage::PlaybackEnd => &mut self.playback_end,
        }
    }
}

fn percentile_from_buckets(buckets: &[u64; N_LATENCY_BUCKETS], pct: u64) -> u64 {
    let total: u64 = buckets.iter().sum();
    if total == 0 {
        return 0;
    }
    let target = (total * pct).div_ceil(100).max(1);
    let mut cumulative = 0u64;
    for (i, &count) in buckets.iter().enumerate() {
        cumulative += count;
        if cumulative >= target {
            return if i < BUCKET_BOUNDARIES_MS.len() {
                BUCKET_BOUNDARIES_MS[i]
            } else {
                BUCKET_BOUNDARIES_MS[BUCKET_BOUNDARIES_MS.len() - 1] + 1
            };
        }
    }
    0
}

fn global_agg() -> &'static Mutex<LiveLatencyAggregator> {
    static AGG: OnceLock<Mutex<LiveLatencyAggregator>> = OnceLock::new();
    AGG.get_or_init(|| Mutex::new(LiveLatencyAggregator::new()))
}

pub(crate) fn record_latency(stage: LatencyStage, ms: u64) {
    if !super::enabled() {
        return;
    }
    let label = match stage {
        LatencyStage::CaptureToRequest => "capture_to_request",
        LatencyStage::Ttfb => "ttfb",
        LatencyStage::FirstAudio => "first_audio",
        LatencyStage::PlaybackEnd => "playback_end",
    };
    super::emit("live_latency", label, &format!("ms={ms}"));
    if let Ok(mut agg) = global_agg().lock() {
        agg.record(stage, ms);
    }
}

pub(crate) fn increment_underruns() {
    if !super::enabled() {
        return;
    }
    if let Ok(mut agg) = global_agg().lock() {
        agg.increment_underruns();
        let total = agg.underruns;
        super::emit("live_latency", "underrun", &format!("total={total}"));
    }
}

pub(crate) fn emit_latency_summary() {
    if !super::enabled() {
        return;
    }
    if let Ok(agg) = global_agg().lock() {
        let stages: &[(LatencyStage, &str)] = &[
            (LatencyStage::CaptureToRequest, "capture_to_request"),
            (LatencyStage::Ttfb, "ttfb"),
            (LatencyStage::FirstAudio, "first_audio"),
            (LatencyStage::PlaybackEnd, "playback_end"),
        ];
        for &(stage, name) in stages {
            super::emit(
                "live_latency_summary",
                name,
                &format!("p50={} p95={}", agg.p50(stage), agg.p95(stage)),
            );
        }
        super::emit(
            "live_latency_summary",
            "underruns",
            &format!("total={}", agg.underruns),
        );
    }
}
