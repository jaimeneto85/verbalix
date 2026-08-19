use crate::{
    application::{AudioCapturePort, AudioStreamPort},
    domain::{EnrollmentSample, MicrophonePermission, VerbalixError},
    platform::audio_processing::{process_audio, resolve_physical_input_device},
};
use cpal::traits::{DeviceTrait, StreamTrait};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;

const MAX_DURATION_SECS: f32 = 120.0;

enum CaptureCommand {
    Start(mpsc::SyncSender<Result<(), VerbalixError>>),
    Stop(mpsc::SyncSender<Result<EnrollmentSample, VerbalixError>>),
    Cancel,
    StartStream {
        sink: Box<dyn Fn(Vec<f32>, u32, u16) + Send + Sync + 'static>,
        error_sink: Box<dyn Fn() + Send + Sync + 'static>,
        reply: mpsc::SyncSender<Result<(), VerbalixError>>,
    },
    StopStream,
}

struct ActiveCapture {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    channels: u16,
    sample_rate: u32,
}

struct ActiveStream {
    _stream: cpal::Stream,
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
            let mut streaming: Option<ActiveStream> = None;

            while let Ok(cmd) = cmd_rx.recv() {
                match cmd {
                    CaptureCommand::Start(reply) => {
                        if streaming.is_some() {
                            let _ = reply.send(Err(VerbalixError::AudioCaptureFailed));
                            continue;
                        }
                        let host = cpal::default_host();
                        let device = match resolve_physical_input_device(&host) {
                            Ok(d) => d,
                            Err(e) => {
                                let _ = reply.send(Err(e));
                                continue;
                            }
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

                    CaptureCommand::StartStream {
                        sink,
                        error_sink,
                        reply,
                    } => {
                        if active.is_some() {
                            let _ = reply.send(Err(VerbalixError::AudioCaptureFailed));
                            continue;
                        }
                        let host = cpal::default_host();
                        let device = match resolve_physical_input_device(&host) {
                            Ok(d) => d,
                            Err(e) => {
                                let _ = reply.send(Err(e));
                                continue;
                            }
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

                        let sink = Arc::new(sink);
                        let sink_cb = Arc::clone(&sink);
                        let error_sink = Arc::new(error_sink);
                        let error_sink_cb = Arc::clone(&error_sink);

                        let stream_result = match config.sample_format() {
                            cpal::SampleFormat::F32 => device.build_input_stream(
                                &config.config(),
                                move |data: &[f32], _| {
                                    sink_cb(data.to_vec(), sample_rate, channels);
                                },
                                move |_err| {
                                    error_sink_cb();
                                },
                                None,
                            ),
                            cpal::SampleFormat::I16 => device.build_input_stream(
                                &config.config(),
                                move |data: &[i16], _| {
                                    let frames: Vec<f32> =
                                        data.iter().map(|&s| s as f32 / 32768.0).collect();
                                    sink_cb(frames, sample_rate, channels);
                                },
                                move |_err| {
                                    error_sink_cb();
                                },
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
                                    streaming = Some(ActiveStream { _stream: stream });
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

                    CaptureCommand::StopStream => {
                        streaming = None;
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
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| VerbalixError::AudioCaptureFailed)?
    }

    fn stop(&self) -> Result<EnrollmentSample, VerbalixError> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.cmd_tx
            .send(CaptureCommand::Stop(tx))
            .map_err(|_| VerbalixError::AudioCaptureFailed)?;
        rx.recv_timeout(Duration::from_secs(70))
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

impl AudioStreamPort for MacAudioCapture {
    fn start_stream(
        &self,
        sink: Box<dyn Fn(Vec<f32>, u32, u16) + Send + Sync + 'static>,
        error_sink: Box<dyn Fn() + Send + Sync + 'static>,
    ) -> Result<(), VerbalixError> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.cmd_tx
            .send(CaptureCommand::StartStream {
                sink,
                error_sink,
                reply: tx,
            })
            .map_err(|_| VerbalixError::AudioCaptureFailed)?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| VerbalixError::AudioCaptureFailed)?
    }

    fn stop_stream(&self) {
        let _ = self.cmd_tx.send(CaptureCommand::StopStream);
    }
}
