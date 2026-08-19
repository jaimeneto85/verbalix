use crate::{
    application::{VirtualMicMetrics, VirtualMicOutputPort},
    domain::VerbalixError,
    platform::virtual_mic_constants::{
        VERBALIX_MIC_CHANNELS, VERBALIX_MIC_DEVICE_NAME, VERBALIX_RING_CAPACITY_FRAMES,
    },
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    collections::VecDeque,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

pub(crate) struct RingBuffer {
    data: VecDeque<f32>,
    capacity: usize,
    underruns: u64,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
            underruns: 0,
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        self.data.extend(samples.iter().copied());
        while self.data.len() > self.capacity {
            self.data.pop_front();
        }
    }

    pub fn pop_into(&mut self, output: &mut [f32]) {
        let mut had_underrun = false;
        for sample in output.iter_mut() {
            match self.data.pop_front() {
                Some(s) => *sample = s,
                None => {
                    *sample = 0.0;
                    had_underrun = true;
                }
            }
        }
        if had_underrun {
            self.underruns += 1;
        }
    }

    pub fn depth(&self) -> usize {
        self.data.len()
    }

    pub fn underruns(&self) -> u64 {
        self.underruns
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }
}

enum StreamCommand {
    Open(mpsc::SyncSender<Result<(), VerbalixError>>),
    Close,
}

pub struct MacVirtualMicOutput {
    ring: Arc<Mutex<RingBuffer>>,
    cmd_tx: mpsc::SyncSender<StreamCommand>,
}

impl MacVirtualMicOutput {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::sync_channel::<StreamCommand>(4);
        let ring = Arc::new(Mutex::new(RingBuffer::new(VERBALIX_RING_CAPACITY_FRAMES)));
        let ring_thread = Arc::clone(&ring);

        std::thread::spawn(move || {
            let mut active_stream: Option<cpal::Stream> = None;

            while let Ok(cmd) = cmd_rx.recv() {
                match cmd {
                    StreamCommand::Open(reply) => {
                        drop(active_stream.take());

                        let host = cpal::default_host();
                        let device = match host.output_devices().ok().and_then(|mut it| {
                            it.find(|d| d.name().unwrap_or_default() == VERBALIX_MIC_DEVICE_NAME)
                        }) {
                            Some(d) => d,
                            None => {
                                let _ = reply.send(Err(VerbalixError::VirtualMicUnavailable));
                                continue;
                            }
                        };

                        let config = match device.default_output_config() {
                            Ok(c) => c,
                            Err(_) => {
                                let _ = reply.send(Err(VerbalixError::VirtualMicUnavailable));
                                continue;
                            }
                        };

                        let ring_cb = Arc::clone(&ring_thread);
                        let stream_result = match config.sample_format() {
                            cpal::SampleFormat::F32 => device.build_output_stream(
                                &config.config(),
                                move |output: &mut [f32], _| {
                                    drain_ring_f32(output, &ring_cb);
                                },
                                |_| {},
                                None,
                            ),
                            cpal::SampleFormat::I16 => device.build_output_stream(
                                &config.config(),
                                move |output: &mut [i16], _| {
                                    drain_ring_i16(output, &ring_cb);
                                },
                                |_| {},
                                None,
                            ),
                            _ => {
                                let _ = reply.send(Err(VerbalixError::VirtualMicUnavailable));
                                continue;
                            }
                        };

                        let stream = match stream_result {
                            Ok(s) => s,
                            Err(_) => {
                                let _ = reply.send(Err(VerbalixError::VirtualMicUnavailable));
                                continue;
                            }
                        };

                        if stream.play().is_err() {
                            let _ = reply.send(Err(VerbalixError::VirtualMicUnavailable));
                            continue;
                        }

                        active_stream = Some(stream);
                        let _ = reply.send(Ok(()));
                    }

                    StreamCommand::Close => {
                        drop(active_stream.take());
                    }
                }
            }
        });

        Self { ring, cmd_tx }
    }
}

fn adapt_to_stereo(samples: Vec<f32>, input_channels: u16) -> Vec<f32> {
    let target = VERBALIX_MIC_CHANNELS as usize;
    let src = input_channels as usize;
    if src == target {
        return samples;
    }
    if src == 1 {
        return samples.iter().flat_map(|&s| [s, s]).collect();
    }
    if src > target {
        let frames = samples.len() / src;
        let mut out = Vec::with_capacity(frames * target);
        for f in 0..frames {
            for c in 0..target {
                out.push(samples[f * src + c]);
            }
        }
        return out;
    }
    samples
}

fn drain_ring_f32(output: &mut [f32], ring: &Arc<Mutex<RingBuffer>>) {
    if let Ok(mut r) = ring.lock() {
        r.pop_into(output);
    }
}

fn drain_ring_i16(output: &mut [i16], ring: &Arc<Mutex<RingBuffer>>) {
    let mut buf = vec![0.0f32; output.len()];
    if let Ok(mut r) = ring.lock() {
        r.pop_into(&mut buf);
    }
    for (out, &f) in output.iter_mut().zip(buf.iter()) {
        *out = (f * 32767.0_f32).clamp(-32768.0, 32767.0) as i16;
    }
}

impl VirtualMicOutputPort for MacVirtualMicOutput {
    fn open(&self) -> Result<(), VerbalixError> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.cmd_tx
            .send(StreamCommand::Open(tx))
            .map_err(|_| VerbalixError::VirtualMicUnavailable)?;
        rx.recv_timeout(Duration::from_secs(5))
            .map_err(|_| VerbalixError::VirtualMicUnavailable)?
    }

    fn enqueue(&self, samples_48k: Vec<f32>, channels: u16) {
        let stereo = adapt_to_stereo(samples_48k, channels);
        if let Ok(mut r) = self.ring.lock() {
            r.push(&stereo);
        }
    }

    fn close(&self) {
        let _ = self.cmd_tx.try_send(StreamCommand::Close);
        if let Ok(mut r) = self.ring.lock() {
            r.clear();
        }
    }

    fn metrics(&self) -> VirtualMicMetrics {
        let (depth, underruns) = self
            .ring
            .lock()
            .map(|r| (r.depth() as u32, r.underruns()))
            .unwrap_or((0, 0));
        VirtualMicMetrics {
            buffer_depth: depth,
            underruns,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RingBuffer;

    #[test]
    fn push_and_pop_basic() {
        let mut ring = RingBuffer::new(10);
        ring.push(&[1.0, 2.0, 3.0]);
        assert_eq!(ring.depth(), 3);
        let mut out = [0.0f32; 2];
        ring.pop_into(&mut out);
        assert_eq!(out, [1.0, 2.0]);
        assert_eq!(ring.depth(), 1);
    }

    #[test]
    fn overflow_drops_oldest() {
        let mut ring = RingBuffer::new(4);
        ring.push(&[1.0, 2.0, 3.0, 4.0]);
        ring.push(&[5.0, 6.0]);
        assert_eq!(ring.depth(), 4);
        let mut out = [0.0f32; 4];
        ring.pop_into(&mut out);
        assert_eq!(out, [3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn underrun_emits_silence_and_counts() {
        let mut ring = RingBuffer::new(10);
        ring.push(&[1.0]);
        let mut out = [0.0f32; 3];
        ring.pop_into(&mut out);
        assert_eq!(out, [1.0, 0.0, 0.0]);
        assert_eq!(ring.underruns(), 1);
    }

    #[test]
    fn underrun_on_empty() {
        let mut ring = RingBuffer::new(10);
        let mut out = [0.0f32; 2];
        ring.pop_into(&mut out);
        assert_eq!(ring.underruns(), 1);
        ring.pop_into(&mut out);
        assert_eq!(ring.underruns(), 2);
    }

    #[test]
    fn no_underrun_when_sufficient() {
        let mut ring = RingBuffer::new(10);
        ring.push(&[1.0, 2.0, 3.0]);
        let mut out = [0.0f32; 3];
        ring.pop_into(&mut out);
        assert_eq!(ring.underruns(), 0);
    }

    #[test]
    fn clear_empties_ring() {
        let mut ring = RingBuffer::new(10);
        ring.push(&[1.0, 2.0]);
        ring.clear();
        assert_eq!(ring.depth(), 0);
    }
}
