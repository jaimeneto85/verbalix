use crate::{
    application::{AudioPreviewPort, VirtualMicOutputPort},
    domain::VerbalixError,
    platform::audio_wav::{decode_wav_f32, resample_f32, VIRTUAL_MIC_SAMPLE_RATE},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct PlaybackRouter {
    speaker: Arc<dyn AudioPreviewPort>,
    virtual_mic: Arc<dyn VirtualMicOutputPort>,
    route: Arc<AtomicBool>,
}

impl PlaybackRouter {
    pub fn new(
        speaker: Arc<dyn AudioPreviewPort>,
        virtual_mic: Arc<dyn VirtualMicOutputPort>,
        route: Arc<AtomicBool>,
    ) -> Self {
        Self {
            speaker,
            virtual_mic,
            route,
        }
    }
}

impl AudioPreviewPort for PlaybackRouter {
    fn play(&self, wav_bytes: Vec<u8>) -> Result<(), VerbalixError> {
        if self.route.load(Ordering::Relaxed) {
            let (samples, src_rate, channels) =
                decode_wav_f32(&wav_bytes).ok_or(VerbalixError::AudioPlaybackFailed)?;
            let resampled = resample_f32(&samples, src_rate, VIRTUAL_MIC_SAMPLE_RATE, channels);
            self.virtual_mic.enqueue(resampled, 1);
            Ok(())
        } else {
            self.speaker.play(wav_bytes)
        }
    }

    fn stop(&self) {
        self.speaker.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{VirtualMicMetrics, VirtualMicOutputPort},
        domain::VerbalixError,
        platform::audio_wav::{encode_wav, TARGET_SAMPLE_RATE},
    };
    use std::sync::atomic::{AtomicU32, Ordering};

    struct SpySpeaker {
        play_count: Arc<AtomicU32>,
        stop_count: Arc<AtomicU32>,
    }

    impl SpySpeaker {
        fn new() -> (Self, Arc<AtomicU32>, Arc<AtomicU32>) {
            let play_count = Arc::new(AtomicU32::new(0));
            let stop_count = Arc::new(AtomicU32::new(0));
            let spy = Self {
                play_count: Arc::clone(&play_count),
                stop_count: Arc::clone(&stop_count),
            };
            (spy, play_count, stop_count)
        }
    }

    impl AudioPreviewPort for SpySpeaker {
        fn play(&self, _wav_bytes: Vec<u8>) -> Result<(), VerbalixError> {
            self.play_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn stop(&self) {
            self.stop_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) struct SpyVirtualMic {
        pub enqueue_count: Arc<AtomicU32>,
        pub open_fails: bool,
    }

    impl SpyVirtualMic {
        pub fn new() -> (Self, Arc<AtomicU32>) {
            let enqueue_count = Arc::new(AtomicU32::new(0));
            let spy = Self {
                enqueue_count: Arc::clone(&enqueue_count),
                open_fails: false,
            };
            (spy, enqueue_count)
        }
    }

    impl VirtualMicOutputPort for SpyVirtualMic {
        fn open(&self) -> Result<(), VerbalixError> {
            if self.open_fails {
                Err(VerbalixError::VirtualMicUnavailable)
            } else {
                Ok(())
            }
        }

        fn enqueue(&self, _samples_48k: Vec<f32>, _channels: u16) {
            self.enqueue_count.fetch_add(1, Ordering::Relaxed);
        }

        fn close(&self) {}

        fn metrics(&self) -> VirtualMicMetrics {
            VirtualMicMetrics {
                buffer_depth: 0,
                underruns: 0,
            }
        }
    }

    fn make_wav() -> Vec<u8> {
        let samples: Vec<i16> = vec![100i16; 32];
        encode_wav(&samples, TARGET_SAMPLE_RATE)
    }

    #[test]
    fn route_off_plays_through_speaker() {
        let route = Arc::new(AtomicBool::new(false));
        let (speaker, play_count, _) = SpySpeaker::new();
        let (vmic, enqueue_count) = SpyVirtualMic::new();
        let router = PlaybackRouter::new(Arc::new(speaker), Arc::new(vmic), Arc::clone(&route));

        router.play(make_wav()).unwrap();

        assert_eq!(play_count.load(Ordering::Relaxed), 1);
        assert_eq!(enqueue_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn route_on_enqueues_to_virtual_mic() {
        let route = Arc::new(AtomicBool::new(true));
        let (speaker, play_count, _) = SpySpeaker::new();
        let (vmic, enqueue_count) = SpyVirtualMic::new();
        let router = PlaybackRouter::new(Arc::new(speaker), Arc::new(vmic), Arc::clone(&route));

        router.play(make_wav()).unwrap();

        assert_eq!(play_count.load(Ordering::Relaxed), 0);
        assert_eq!(enqueue_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn stop_always_calls_speaker_stop() {
        let route = Arc::new(AtomicBool::new(true));
        let (speaker, _, stop_count) = SpySpeaker::new();
        let (vmic, _) = SpyVirtualMic::new();
        let router = PlaybackRouter::new(Arc::new(speaker), Arc::new(vmic), route);

        router.stop();

        assert_eq!(stop_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn route_on_with_invalid_wav_returns_error() {
        let route = Arc::new(AtomicBool::new(true));
        let (speaker, _, _) = SpySpeaker::new();
        let (vmic, _) = SpyVirtualMic::new();
        let router = PlaybackRouter::new(Arc::new(speaker), Arc::new(vmic), route);

        let result = router.play(vec![0u8; 10]);
        assert!(matches!(result, Err(VerbalixError::AudioPlaybackFailed)));
    }
}
