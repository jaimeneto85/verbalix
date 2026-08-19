use crate::{
    application::AudioCapturePort,
    domain::{EnrollmentSample, MicrophonePermission, VerbalixError},
    platform::audio_wav::{encode_wav, resample_to_16k, TARGET_SAMPLE_RATE},
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    mpsc, Arc, Mutex,
};

const MAX_DURATION_SECS: f32 = 120.0;
const MIN_DURATION_SECS: f32 = 5.0;

enum CaptureCommand {
    Start(mpsc::SyncSender<Result<(), VerbalixError>>),
    Stop(mpsc::SyncSender<Result<EnrollmentSample, VerbalixError>>),
    Cancel,
}

struct ActiveCapture {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    channels: u16,
    sample_rate: u32,
}

pub struct MacAudioCapture {
    cmd_tx: mpsc::SyncSender<CaptureCommand>,
    level: Arc<AtomicU32>,
}

impl MacAudioCapture {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::sync_channel::<CaptureCommand>(4);
        let level = Arc::new(AtomicU32::new(0));
        let level_thread = Arc::clone(&level);

        std::thread::spawn(move || {
            let mut active: Option<ActiveCapture> = None;

            while let Ok(cmd) = cmd_rx.recv() {
                match cmd {
                    CaptureCommand::Start(reply) => {
                        let host = cpal::default_host();
                        let Some(device) = host.default_input_device() else {
                            let _ = reply.send(Err(VerbalixError::AudioCaptureFailed));
                            continue;
                        };
                        let config = match device.default_input_config() {
                            Ok(c) => c,
                            Err(_) => {
                                let _ = reply.send(Err(VerbalixError::AudioCaptureFailed));
                                continue;
                            }
                        };
                        let sample_rate = config.sample_rate().0;
                        let channels = config.channels();
                        let max_cap =
                            (sample_rate as f32 * channels as f32 * MAX_DURATION_SECS) as usize;

                        let buffer: Arc<Mutex<Vec<f32>>> =
                            Arc::new(Mutex::new(Vec::with_capacity(max_cap)));
                        let buf_cb = Arc::clone(&buffer);
                        let level_cb = Arc::clone(&level_thread);

                        let stream_result = match config.sample_format() {
                            cpal::SampleFormat::F32 => device.build_input_stream(
                                &config.config(),
                                move |data: &[f32], _| {
                                    let sum: f32 = data.iter().map(|s| s * s).sum();
                                    let rms = (sum / data.len().max(1) as f32).sqrt();
                                    level_cb.store(rms.to_bits(), Ordering::Relaxed);
                                    if let Ok(mut buf) = buf_cb.lock() {
                                        if buf.len() < max_cap {
                                            let remaining = max_cap - buf.len();
                                            let take = data.len().min(remaining);
                                            buf.extend_from_slice(&data[..take]);
                                        }
                                    }
                                },
                                |_| {},
                                None,
                            ),
                            cpal::SampleFormat::I16 => device.build_input_stream(
                                &config.config(),
                                move |data: &[i16], _| {
                                    let sum: f32 = data
                                        .iter()
                                        .map(|&s| {
                                            let f = s as f32 / 32768.0;
                                            f * f
                                        })
                                        .sum();
                                    let rms = (sum / data.len().max(1) as f32).sqrt();
                                    level_cb.store(rms.to_bits(), Ordering::Relaxed);
                                    if let Ok(mut buf) = buf_cb.lock() {
                                        if buf.len() < max_cap {
                                            let remaining = max_cap - buf.len();
                                            let take = data.len().min(remaining);
                                            buf.extend(
                                                data[..take].iter().map(|&s| s as f32 / 32768.0),
                                            );
                                        }
                                    }
                                },
                                |_| {},
                                None,
                            ),
                            _ => {
                                let _ = reply.send(Err(VerbalixError::AudioCaptureFailed));
                                continue;
                            }
                        };

                        match stream_result {
                            Ok(stream) => match stream.play() {
                                Ok(()) => {
                                    active = Some(ActiveCapture {
                                        _stream: stream,
                                        buffer,
                                        channels,
                                        sample_rate,
                                    });
                                    let _ = reply.send(Ok(()));
                                }
                                Err(_) => {
                                    let _ = reply.send(Err(VerbalixError::AudioCaptureFailed));
                                }
                            },
                            Err(_) => {
                                let _ = reply.send(Err(VerbalixError::AudioCaptureFailed));
                            }
                        }
                    }

                    CaptureCommand::Stop(reply) => {
                        let result = if let Some(cap) = active.take() {
                            drop(cap._stream);
                            let data = cap.buffer.lock().map(|b| b.clone()).unwrap_or_default();
                            process_audio(data, cap.channels, cap.sample_rate)
                        } else {
                            Err(VerbalixError::AudioCaptureFailed)
                        };
                        level_thread.store(0f32.to_bits(), Ordering::Relaxed);
                        let _ = reply.send(result);
                    }

                    CaptureCommand::Cancel => {
                        active = None;
                        level_thread.store(0f32.to_bits(), Ordering::Relaxed);
                    }
                }
            }
        });

        Self { cmd_tx, level }
    }
}

impl AudioCapturePort for MacAudioCapture {
    fn start(&self) -> Result<(), VerbalixError> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.cmd_tx
            .send(CaptureCommand::Start(tx))
            .map_err(|_| VerbalixError::AudioCaptureFailed)?;
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| VerbalixError::AudioCaptureFailed)?
    }

    fn stop(&self) -> Result<EnrollmentSample, VerbalixError> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.cmd_tx
            .send(CaptureCommand::Stop(tx))
            .map_err(|_| VerbalixError::AudioCaptureFailed)?;
        rx.recv_timeout(std::time::Duration::from_secs(70))
            .map_err(|_| VerbalixError::AudioCaptureFailed)?
    }

    fn cancel(&self) {
        let _ = self.cmd_tx.send(CaptureCommand::Cancel);
    }

    fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    fn permission_status(&self) -> MicrophonePermission {
        crate::platform::microphone_permission_status()
    }
}

fn process_audio(
    raw: Vec<f32>,
    channels: u16,
    native_rate: u32,
) -> Result<EnrollmentSample, VerbalixError> {
    if raw.is_empty() {
        return Err(VerbalixError::AudioCaptureFailed);
    }

    let total_frames = raw.len() / channels.max(1) as usize;
    let duration_secs = total_frames as f32 / native_rate.max(1) as f32;

    if duration_secs < MIN_DURATION_SECS {
        return Err(VerbalixError::AudioCaptureFailed);
    }

    let samples_i16 = resample_to_16k(&raw, native_rate, channels);
    let wav_bytes = encode_wav(&samples_i16, TARGET_SAMPLE_RATE);

    Ok(EnrollmentSample { wav_bytes })
}

