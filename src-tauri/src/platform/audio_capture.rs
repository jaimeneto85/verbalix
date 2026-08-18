use crate::{
    application::AudioCapturePort,
    domain::{EnrollmentSample, MicrophonePermission, VerbalixError},
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    mpsc, Arc, Mutex,
};

const MAX_DURATION_SECS: f32 = 120.0;
const TARGET_SAMPLE_RATE: u32 = 16_000;

enum CaptureCommand {
    Start,
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
                    CaptureCommand::Start => {
                        let host = cpal::default_host();
                        let Some(device) = host.default_input_device() else {
                            continue;
                        };
                        let config = match device.default_input_config() {
                            Ok(c) => c,
                            Err(_) => continue,
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
                            _ => continue,
                        };

                        if let Ok(stream) = stream_result {
                            let _ = stream.play();
                            active = Some(ActiveCapture {
                                _stream: stream,
                                buffer,
                                channels,
                                sample_rate,
                            });
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
        self.cmd_tx
            .send(CaptureCommand::Start)
            .map_err(|_| VerbalixError::AudioCaptureFailed)
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

    let samples_i16 = resample_to_16k(&raw, native_rate, channels);
    let wav_bytes = encode_wav(&samples_i16, TARGET_SAMPLE_RATE);

    Ok(EnrollmentSample {
        wav_bytes,
        duration_secs,
    })
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
