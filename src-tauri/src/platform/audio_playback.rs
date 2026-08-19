#[cfg(target_os = "macos")]
mod mac {
    use crate::{application::AudioPreviewPort, domain::VerbalixError};
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::{
        collections::VecDeque,
        sync::{mpsc, Arc, Mutex},
        time::{Duration, Instant},
    };

    enum PlaybackCommand {
        Play(Vec<u8>, mpsc::SyncSender<Result<(), VerbalixError>>),
        Stop,
    }

    pub struct MacAudioPlayback {
        cmd_tx: mpsc::SyncSender<PlaybackCommand>,
    }

    impl MacAudioPlayback {
        pub fn new() -> Self {
            let (cmd_tx, cmd_rx) = mpsc::sync_channel::<PlaybackCommand>(4);

            std::thread::spawn(move || {
                let mut active_stream: Option<cpal::Stream> = None;

                while let Ok(cmd) = cmd_rx.recv() {
                    match cmd {
                        PlaybackCommand::Play(wav_bytes, reply) => {
                            drop(active_stream.take());

                            let samples = match decode_wav_f32(&wav_bytes) {
                                Some(s) => s,
                                None => {
                                    let _ = reply.send(Err(VerbalixError::AudioPlaybackFailed));
                                    continue;
                                }
                            };

                            let host = cpal::default_host();
                            let device = match host.default_output_device() {
                                Some(d) => d,
                                None => {
                                    let _ = reply.send(Err(VerbalixError::AudioPlaybackFailed));
                                    continue;
                                }
                            };
                            let config = match device.default_output_config() {
                                Ok(c) => c,
                                Err(_) => {
                                    let _ = reply.send(Err(VerbalixError::AudioPlaybackFailed));
                                    continue;
                                }
                            };

                            let buffer: Arc<Mutex<VecDeque<f32>>> =
                                Arc::new(Mutex::new(VecDeque::from(samples)));
                            let buf_cb = Arc::clone(&buffer);
                            let (done_tx, done_rx) = mpsc::sync_channel::<()>(1);
                            let done_tx = Arc::new(Mutex::new(Some(done_tx)));
                            let done_tx_cb = Arc::clone(&done_tx);

                            let stream_result = match config.sample_format() {
                                cpal::SampleFormat::F32 => device.build_output_stream(
                                    &config.config(),
                                    move |output: &mut [f32], _| {
                                        fill_output_f32(output, &buf_cb, &done_tx_cb);
                                    },
                                    |_| {},
                                    None,
                                ),
                                cpal::SampleFormat::I16 => device.build_output_stream(
                                    &config.config(),
                                    move |output: &mut [i16], _| {
                                        fill_output_i16(output, &buf_cb, &done_tx_cb);
                                    },
                                    |_| {},
                                    None,
                                ),
                                _ => {
                                    let _ = reply.send(Err(VerbalixError::AudioPlaybackFailed));
                                    continue;
                                }
                            };

                            let stream = match stream_result {
                                Ok(s) => s,
                                Err(_) => {
                                    let _ = reply.send(Err(VerbalixError::AudioPlaybackFailed));
                                    continue;
                                }
                            };

                            if stream.play().is_err() {
                                let _ = reply.send(Err(VerbalixError::AudioPlaybackFailed));
                                continue;
                            }

                            active_stream = Some(stream);

                            let deadline = Instant::now() + Duration::from_secs(60);
                            let result = loop {
                                match done_rx.try_recv() {
                                    Ok(()) | Err(mpsc::TryRecvError::Disconnected) => break Ok(()),
                                    Err(mpsc::TryRecvError::Empty) => {}
                                }
                                if Instant::now() >= deadline {
                                    break Err(VerbalixError::AudioPlaybackFailed);
                                }
                                match cmd_rx.try_recv() {
                                    Ok(PlaybackCommand::Stop) => {
                                        break Err(VerbalixError::AudioPlaybackFailed);
                                    }
                                    Ok(PlaybackCommand::Play(_, r)) => {
                                        let _ = r.send(Err(VerbalixError::AudioPlaybackFailed));
                                    }
                                    Err(_) => {}
                                }
                                std::thread::sleep(Duration::from_millis(5));
                            };
                            drop(active_stream.take());
                            let _ = reply.send(result);
                        }

                        PlaybackCommand::Stop => {
                            drop(active_stream.take());
                        }
                    }
                }
            });

            Self { cmd_tx }
        }
    }

    impl AudioPreviewPort for MacAudioPlayback {
        fn play(&self, wav_bytes: Vec<u8>) -> Result<(), VerbalixError> {
            let (tx, rx) = mpsc::sync_channel(1);
            self.cmd_tx
                .send(PlaybackCommand::Play(wav_bytes, tx))
                .map_err(|_| VerbalixError::AudioPlaybackFailed)?;
            rx.recv_timeout(Duration::from_secs(65))
                .map_err(|_| VerbalixError::AudioPlaybackFailed)?
        }

        fn stop(&self) {
            let _ = self.cmd_tx.send(PlaybackCommand::Stop);
        }
    }

    fn fill_output_f32(
        output: &mut [f32],
        buffer: &Arc<Mutex<VecDeque<f32>>>,
        done_tx: &Arc<Mutex<Option<mpsc::SyncSender<()>>>>,
    ) {
        if let Ok(mut buf) = buffer.lock() {
            for sample in output.iter_mut() {
                *sample = buf.pop_front().unwrap_or(0.0);
            }
            if buf.is_empty() {
                if let Ok(mut guard) = done_tx.lock() {
                    if let Some(tx) = guard.take() {
                        let _ = tx.send(());
                    }
                }
            }
        }
    }

    fn fill_output_i16(
        output: &mut [i16],
        buffer: &Arc<Mutex<VecDeque<f32>>>,
        done_tx: &Arc<Mutex<Option<mpsc::SyncSender<()>>>>,
    ) {
        if let Ok(mut buf) = buffer.lock() {
            for sample in output.iter_mut() {
                let f = buf.pop_front().unwrap_or(0.0);
                *sample = (f * 32767.0).clamp(-32768.0, 32767.0) as i16;
            }
            if buf.is_empty() {
                if let Ok(mut guard) = done_tx.lock() {
                    if let Some(tx) = guard.take() {
                        let _ = tx.send(());
                    }
                }
            }
        }
    }

    fn decode_wav_f32(bytes: &[u8]) -> Option<Vec<f32>> {
        if bytes.len() < 44 {
            return None;
        }
        if &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return None;
        }
        let channels = u16::from_le_bytes([bytes[22], bytes[23]]) as usize;
        let bits_per_sample = u16::from_le_bytes([bytes[34], bytes[35]]);
        let data_size = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]) as usize;
        let data = bytes.get(44..44 + data_size)?;

        if bits_per_sample != 16 || channels == 0 {
            return None;
        }

        let samples: Vec<f32> = data
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect();

        Some(samples)
    }
}

#[cfg(target_os = "macos")]
pub use mac::MacAudioPlayback;
