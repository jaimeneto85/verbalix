use crate::{
    domain::{EnrollmentSample, VerbalixError},
    platform::{
        audio_wav::{encode_wav, resample_to_16k, TARGET_SAMPLE_RATE},
        virtual_mic_constants::VERBALIX_MIC_DEVICE_NAME,
    },
};
use cpal::traits::{DeviceTrait, HostTrait};

const MIN_DURATION_SECS: f32 = 5.0;

pub(crate) fn resolve_physical_input_device(
    host: &cpal::Host,
) -> Result<cpal::Device, VerbalixError> {
    let default = host
        .default_input_device()
        .ok_or(VerbalixError::AudioCaptureFailed)?;
    let default_name = default.name().unwrap_or_default();
    if !default_name.starts_with(VERBALIX_MIC_DEVICE_NAME) {
        return Ok(default);
    }
    host.input_devices()
        .map_err(|_| VerbalixError::AudioCaptureFailed)?
        .find(|d| {
            !d.name()
                .unwrap_or_default()
                .starts_with(VERBALIX_MIC_DEVICE_NAME)
        })
        .ok_or(VerbalixError::VirtualMicSelectedAsInput)
}

pub(crate) fn process_audio(
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
