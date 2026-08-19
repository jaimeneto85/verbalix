pub const TARGET_SAMPLE_RATE: u32 = 16_000;

pub fn pcm_i16le_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect()
}

pub struct IncrementalResampler {
    src_rate: u32,
    dst_rate: u32,
    channels: u16,
    ratio: f64,
    phase: f64,
}

impl IncrementalResampler {
    pub fn new(src_rate: u32, dst_rate: u32, channels: u16) -> Self {
        Self {
            ratio: src_rate as f64 / dst_rate as f64,
            src_rate,
            dst_rate,
            channels,
            phase: 0.0,
        }
    }

    pub fn push(&mut self, input: &[f32]) -> Vec<f32> {
        let ch = self.channels.max(1) as usize;
        let mono: Vec<f32> = input
            .chunks(ch)
            .map(|frame| frame.iter().sum::<f32>() / ch as f32)
            .collect();

        if mono.is_empty() {
            return Vec::new();
        }

        if self.src_rate == self.dst_rate {
            return mono;
        }

        let mut output = Vec::new();

        while self.phase < mono.len() as f64 {
            let idx = self.phase as usize;
            let frac = (self.phase - idx as f64) as f32;
            let a = mono[idx];
            let b = mono.get(idx + 1).copied().unwrap_or(a);
            output.push(a + (b - a) * frac);
            self.phase += self.ratio;
        }

        self.phase -= mono.len() as f64;
        output
    }
}
pub const VIRTUAL_MIC_SAMPLE_RATE: u32 = 48_000;

pub fn resample_f32(samples: &[f32], src_rate: u32, target_rate: u32, channels: u16) -> Vec<f32> {
    let ch = channels.max(1) as usize;
    let mono: Vec<f32> = samples
        .chunks(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect();

    if src_rate == target_rate {
        return mono;
    }

    let ratio = src_rate as f64 / target_rate as f64;
    let out_len = (mono.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos as usize;
        let frac = (pos - idx as f64) as f32;
        let a = mono.get(idx).copied().unwrap_or(0.0);
        let b = mono.get(idx + 1).copied().unwrap_or(a);
        output.push(a + (b - a) * frac);
    }

    output
}

pub fn resample_to_16k(samples: &[f32], src_rate: u32, channels: u16) -> Vec<i16> {
    resample_f32(samples, src_rate, TARGET_SAMPLE_RATE, channels)
        .iter()
        .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
        .collect()
}

pub fn decode_wav_f32(bytes: &[u8]) -> Option<(Vec<f32>, u32, u16)> {
    if bytes.len() < 44 {
        return None;
    }
    if &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let channels = u16::from_le_bytes([bytes[22], bytes[23]]);
    let sample_rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
    let bits_per_sample = u16::from_le_bytes([bytes[34], bytes[35]]);
    let data_size = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]) as usize;
    let data = bytes.get(44..44 + data_size)?;

    if bits_per_sample != 16 || channels == 0 {
        return None;
    }

    Some((pcm_i16le_to_f32(data), sample_rate, channels))
}

pub fn encode_wav(samples: &[i16], sample_rate: u32) -> Vec<u8> {
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

pub fn pcm_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_f32_identity_when_same_rate() {
        let samples = vec![0.1f32, 0.2, 0.3];
        let out = resample_f32(&samples, 16_000, 16_000, 1);
        assert_eq!(out.len(), 3);
        assert!((out[0] - 0.1).abs() < 1e-5);
    }

    #[test]
    fn resample_f32_upsample() {
        let samples = vec![0.0f32, 1.0];
        let out = resample_f32(&samples, 16_000, 48_000, 1);
        assert!(out.len() > 2);
    }

    #[test]
    fn resample_to_16k_delegates_correctly() {
        let samples = vec![0.5f32; 160];
        let out = resample_to_16k(&samples, 16_000, 1);
        assert_eq!(out.len(), 160);
        assert!(out.iter().all(|&s| s > 0));
    }

    #[test]
    fn resample_f32_mono_mix() {
        let stereo = vec![0.4f32, 0.6, 0.4, 0.6];
        let out = resample_f32(&stereo, 16_000, 16_000, 2);
        assert_eq!(out.len(), 2);
        assert!((out[0] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn decode_wav_f32_round_trips() {
        let samples_i16: Vec<i16> = vec![1000, -1000, 2000, -2000];
        let wav = encode_wav(&samples_i16, 16_000);
        let (decoded, rate, ch) = decode_wav_f32(&wav).unwrap();
        assert_eq!(rate, 16_000);
        assert_eq!(ch, 1);
        assert_eq!(decoded.len(), 4);
        assert!((decoded[0] - 1000.0 / 32768.0).abs() < 1e-4);
    }

    #[test]
    fn decode_wav_f32_rejects_short_bytes() {
        assert!(decode_wav_f32(&[0u8; 10]).is_none());
    }

    #[test]
    fn decode_wav_f32_rejects_non_wav() {
        let mut bytes = vec![0u8; 44];
        bytes[0..4].copy_from_slice(b"XXXX");
        assert!(decode_wav_f32(&bytes).is_none());
    }

    #[test]
    fn pcm_i16le_to_f32_converts_known_values() {
        let pos: i16 = 16384;
        let neg: i16 = -16384;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&pos.to_le_bytes());
        bytes.extend_from_slice(&neg.to_le_bytes());
        let out = pcm_i16le_to_f32(&bytes);
        assert_eq!(out.len(), 2);
        assert!((out[0] - 0.5).abs() < 1e-4);
        assert!((out[1] + 0.5).abs() < 1e-4);
    }

    #[test]
    fn pcm_i16le_to_f32_discards_trailing_odd_byte() {
        let sample: i16 = 32767;
        let mut bytes = sample.to_le_bytes().to_vec();
        bytes.push(0xFF);
        let out = pcm_i16le_to_f32(&bytes);
        assert_eq!(
            out.len(),
            1,
            "trailing odd byte is discarded, not corrupted"
        );
        assert!((out[0] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn incremental_resampler_identity_same_rate() {
        let mut rs = IncrementalResampler::new(16_000, 16_000, 1);
        let input = vec![0.1f32, 0.5, -0.3];
        let out = rs.push(&input);
        assert_eq!(out.len(), 3);
        assert!((out[1] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn ca13_chunked_resample_continuity_at_boundary() {
        let src_rate = 24_000u32;
        let dst_rate = 48_000u32;
        let n = 20usize;
        let half = n / 2;

        let signal: Vec<f32> = (0..n)
            .map(|i| (i as f32 * std::f32::consts::PI / 4.0).sin())
            .collect();

        let full_out = resample_f32(&signal, src_rate, dst_rate, 1);

        let mut rs = IncrementalResampler::new(src_rate, dst_rate, 1);
        let chunk1 = rs.push(&signal[..half]);
        let chunk2 = rs.push(&signal[half..]);

        assert_eq!(
            chunk1.len() + chunk2.len(),
            full_out.len(),
            "total sample count must match"
        );

        let c1_len = chunk1.len();
        for (i, (&full, &got)) in full_out[c1_len..].iter().zip(chunk2.iter()).enumerate() {
            assert!(
                (full - got).abs() < 1e-3,
                "chunk2 sample {i}: full={full:.5} chunked={got:.5} — phase must be continuous across boundary"
            );
        }

        let interior = c1_len.saturating_sub(1);
        for (i, (&full, &got)) in full_out[..interior]
            .iter()
            .zip(chunk1[..interior].iter())
            .enumerate()
        {
            assert!(
                (full - got).abs() < 1e-3,
                "chunk1 interior sample {i}: full={full:.5} chunked={got:.5}"
            );
        }
    }

    #[test]
    fn incremental_resampler_phase_advances_correctly_across_chunks() {
        let src_rate = 24_000u32;
        let dst_rate = 32_000u32;
        let signal: Vec<f32> = (0..12).map(|i| i as f32 / 12.0).collect();

        let full_out = resample_f32(&signal, src_rate, dst_rate, 1);

        let mut rs = IncrementalResampler::new(src_rate, dst_rate, 1);
        let c1 = rs.push(&signal[..4]);
        let c2 = rs.push(&signal[4..8]);
        let c3 = rs.push(&signal[8..]);
        let chunked: Vec<f32> = c1
            .iter()
            .chain(c2.iter())
            .chain(c3.iter())
            .copied()
            .collect();

        assert_eq!(
            chunked.len(),
            full_out.len(),
            "total sample count must match"
        );

        let c1_interior = c1.len().saturating_sub(1);
        for (i, (&full, &got)) in full_out[..c1_interior]
            .iter()
            .zip(chunked[..c1_interior].iter())
            .enumerate()
        {
            assert!(
                (full - got).abs() < 1e-3,
                "c1 interior sample {i}: diff={:.5}",
                (full - got).abs()
            );
        }

        let c2_start = c1.len();
        let c2_interior_end = (c2_start + c2.len()).saturating_sub(1);
        for (i, (&full, &got)) in full_out[c2_start..c2_interior_end]
            .iter()
            .zip(chunked[c2_start..c2_interior_end].iter())
            .enumerate()
        {
            assert!(
                (full - got).abs() < 1e-3,
                "c2 interior sample {i}: diff={:.5}",
                (full - got).abs()
            );
        }

        let c3_start = c1.len() + c2.len();
        for (i, (&full, &got)) in full_out[c3_start..]
            .iter()
            .zip(chunked[c3_start..].iter())
            .enumerate()
        {
            assert!(
                (full - got).abs() < 1e-3,
                "c3 sample {i}: diff={:.5}",
                (full - got).abs()
            );
        }
    }
}
