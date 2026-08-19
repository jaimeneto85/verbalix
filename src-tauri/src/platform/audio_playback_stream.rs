#[cfg(target_os = "macos")]
pub(crate) mod mac_stream {
    use crate::{
        application::streaming_audio::StreamSegmentHandle,
        diagnostics,
        domain::VerbalixError,
        platform::audio_resample::IncrementalResampler,
    };
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::{
        collections::VecDeque,
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc, Mutex,
        },
        time::{Duration, Instant},
    };

    pub(crate) const PRE_BUFFER_SAMPLES_24K: usize = 200 * 24_000 / 1000;

    fn stream_fill_f32(
        output: &mut [f32],
        device_buf: &Arc<Mutex<VecDeque<f32>>>,
        done_flag: &Arc<AtomicBool>,
        done_tx: &Arc<Mutex<Option<mpsc::SyncSender<()>>>>,
    ) {
        if let Ok(mut buf) = device_buf.lock() {
            let mut underrun = false;
            for sample in output.iter_mut() {
                match buf.pop_front() {
                    Some(s) => *sample = s,
                    None => {
                        *sample = 0.0;
                        if !done_flag.load(Ordering::Relaxed) {
                            underrun = true;
                        }
                    }
                }
            }
            if underrun {
                diagnostics::increment_underruns();
            }
            if buf.is_empty() && done_flag.load(Ordering::Relaxed) {
                if let Ok(mut guard) = done_tx.lock() {
                    if let Some(tx) = guard.take() {
                        let _ = tx.send(());
                    }
                }
            }
        }
    }

    fn stream_fill_i16(
        output: &mut [i16],
        device_buf: &Arc<Mutex<VecDeque<f32>>>,
        done_flag: &Arc<AtomicBool>,
        done_tx: &Arc<Mutex<Option<mpsc::SyncSender<()>>>>,
    ) {
        if let Ok(mut buf) = device_buf.lock() {
            let mut underrun = false;
            for sample in output.iter_mut() {
                match buf.pop_front() {
                    Some(f) => *sample = (f * 32767.0).clamp(-32768.0, 32767.0) as i16,
                    None => {
                        *sample = 0;
                        if !done_flag.load(Ordering::Relaxed) {
                            underrun = true;
                        }
                    }
                }
            }
            if underrun {
                diagnostics::increment_underruns();
            }
            if buf.is_empty() && done_flag.load(Ordering::Relaxed) {
                if let Ok(mut guard) = done_tx.lock() {
                    if let Some(tx) = guard.take() {
                        let _ = tx.send(());
                    }
                }
            }
        }
    }

    pub(crate) fn run_stream_playback(handle: StreamSegmentHandle) -> Result<(), VerbalixError> {
        loop {
            if handle.cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            let buf_len = handle.buffer.lock().unwrap().len();
            let complete = handle.complete.load(Ordering::Relaxed);
            if buf_len >= PRE_BUFFER_SAMPLES_24K || complete {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if handle.cancel.load(Ordering::Relaxed) {
            return Ok(());
        }

        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => return Err(VerbalixError::AudioPlaybackFailed),
        };
        let config = match device.default_output_config() {
            Ok(c) => c,
            Err(_) => return Err(VerbalixError::AudioPlaybackFailed),
        };
        let device_rate = config.sample_rate().0;

        let device_buf: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));
        let done_flag: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let (done_tx, done_rx) = mpsc::sync_channel::<()>(1);
        let done_tx_shared = Arc::new(Mutex::new(Some(done_tx)));

        let feed_src = Arc::clone(&handle.buffer);
        let feed_complete = Arc::clone(&handle.complete);
        let feed_cancel = Arc::clone(&handle.cancel);
        let feed_buf = Arc::clone(&device_buf);
        let feed_done = Arc::clone(&done_flag);

        std::thread::spawn(move || {
            let mut resampler = IncrementalResampler::new(24_000, device_rate, 1);
            loop {
                if feed_cancel.load(Ordering::Relaxed) {
                    feed_done.store(true, Ordering::Relaxed);
                    break;
                }
                let chunk: Vec<f32> = {
                    let mut src = feed_src.lock().unwrap();
                    src.drain(..).collect()
                };
                if !chunk.is_empty() {
                    let out = resampler.push(&chunk);
                    feed_buf.lock().unwrap().extend(out);
                }
                let complete = feed_complete.load(Ordering::Relaxed);
                let src_empty = feed_src.lock().unwrap().is_empty();
                if complete && src_empty {
                    feed_done.store(true, Ordering::Relaxed);
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        });

        let db_f32 = Arc::clone(&device_buf);
        let df_f32 = Arc::clone(&done_flag);
        let dt_f32 = Arc::clone(&done_tx_shared);
        let db_i16 = Arc::clone(&device_buf);
        let df_i16 = Arc::clone(&done_flag);
        let dt_i16 = Arc::clone(&done_tx_shared);

        let stream_result = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config.config(),
                move |out: &mut [f32], _| stream_fill_f32(out, &db_f32, &df_f32, &dt_f32),
                |_| {},
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                &config.config(),
                move |out: &mut [i16], _| stream_fill_i16(out, &db_i16, &df_i16, &dt_i16),
                |_| {},
                None,
            ),
            _ => return Err(VerbalixError::AudioPlaybackFailed),
        };

        let stream = match stream_result {
            Ok(s) => s,
            Err(_) => return Err(VerbalixError::AudioPlaybackFailed),
        };

        if stream.play().is_err() {
            return Err(VerbalixError::AudioPlaybackFailed);
        }

        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            match done_rx.try_recv() {
                Ok(()) | Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) => {}
            }
            if handle.cancel.load(Ordering::Relaxed) {
                break;
            }
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        drop(stream);
        Ok(())
    }
}
