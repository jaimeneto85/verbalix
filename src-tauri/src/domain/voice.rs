use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceProfileStatus {
    Enrolling,
    Ready,
    Failed,
    Deleting,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceProfileView {
    pub voice_profile_id: Uuid,
    pub status: VoiceProfileStatus,
    pub display_name: String,
}

pub struct EnrollmentSample {
    pub wav_bytes: Vec<u8>,
    #[allow(dead_code)]
    pub duration_secs: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MicrophonePermission {
    Authorized,
    Denied,
    NotDetermined,
    Restricted,
}
