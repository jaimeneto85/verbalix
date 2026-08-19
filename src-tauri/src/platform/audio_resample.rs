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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::audio_wav::resample_f32;

    #[test]
    fn incremental_resampler_empty_push_returns_empty() {
        let mut rs = IncrementalResampler::new(24_000, 48_000, 1);
        let out = rs.push(&[]);
        assert!(
            out.is_empty(),
            "empty push must return empty vec without panicking"
        );
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
