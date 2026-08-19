pub const VERBALIX_MIC_DEVICE_NAME: &str = "Verbalix Microphone";
pub const VERBALIX_MIC_DEVICE_UID: &str = "com.verbalix.virtualmic:0";
pub const VERBALIX_DRIVER_BUNDLE_ID: &str = "com.verbalix.virtualmic";
pub const VERBALIX_DRIVER_HAL_PATH: &str = "/Library/Audio/Plug-Ins/HAL/VerbalixMicrophone.driver";
pub const VERBALIX_DRIVER_EXPECTED_VERSION: &str = "1.0";
pub const VERBALIX_MIC_SAMPLE_RATE: u32 = 48_000;
pub const VERBALIX_MIC_CHANNELS: u16 = 2;
pub const VERBALIX_RING_CAPACITY_FRAMES: usize = 48_000 * 2 * 2;
