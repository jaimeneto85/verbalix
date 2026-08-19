use crate::{
    application::{VirtualMicDevicePort, VirtualMicStatus},
    platform::virtual_mic_constants::{VERBALIX_DRIVER_EXPECTED_VERSION, VERBALIX_DRIVER_HAL_PATH},
};
use coreaudio_sys::{
    kAudioHardwarePropertyDevices, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject, AudioObjectAddPropertyListener,
    AudioObjectID, AudioObjectPropertyAddress, AudioObjectRemovePropertyListener, OSStatus, UInt32,
};
use std::{ffi::c_void, sync::Mutex};

pub(crate) fn classify_driver_version(
    installed_version: Option<String>,
    expected: &str,
) -> VirtualMicStatus {
    match installed_version {
        None => VirtualMicStatus::NotInstalled,
        Some(v) if v == expected => VirtualMicStatus::Installed,
        Some(_) => VirtualMicStatus::IncompatibleVersion,
    }
}

fn read_bundle_version(plist_path: &str) -> Option<String> {
    let content = std::fs::read_to_string(plist_path).ok()?;
    let tag = "<key>CFBundleVersion</key>";
    let pos = content.find(tag)?;
    let after = &content[pos + tag.len()..];
    let start = after.find("<string>")? + "<string>".len();
    let end = after[start..].find("</string>")?;
    Some(after[start..start + end].trim().to_string())
}

fn current_driver_status() -> VirtualMicStatus {
    let plist = format!("{}/Contents/Info.plist", VERBALIX_DRIVER_HAL_PATH);
    classify_driver_version(
        read_bundle_version(&plist),
        VERBALIX_DRIVER_EXPECTED_VERSION,
    )
}

struct WatchData {
    on_change: Box<dyn Fn(VirtualMicStatus) + Send + Sync>,
}

unsafe extern "C" fn device_list_listener(
    _object_id: AudioObjectID,
    _num_addresses: UInt32,
    _addresses: *const AudioObjectPropertyAddress,
    client_data: *mut c_void,
) -> OSStatus {
    let data = &*(client_data as *const WatchData);
    let status = current_driver_status();
    (data.on_change)(status);
    0
}

fn hardware_devices_address() -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDevices,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    }
}

pub struct MacVirtualMicDevice {
    watch_ptr: Mutex<Option<*mut WatchData>>,
}

unsafe impl Send for MacVirtualMicDevice {}
unsafe impl Sync for MacVirtualMicDevice {}

impl MacVirtualMicDevice {
    pub fn new() -> Self {
        Self {
            watch_ptr: Mutex::new(None),
        }
    }
}

impl Drop for MacVirtualMicDevice {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.watch_ptr.lock() {
            if let Some(ptr) = guard.take() {
                let address = hardware_devices_address();
                unsafe {
                    AudioObjectRemovePropertyListener(
                        kAudioObjectSystemObject,
                        &address,
                        Some(device_list_listener),
                        ptr as *mut c_void,
                    );
                    drop(Box::from_raw(ptr));
                }
            }
        }
    }
}

impl VirtualMicDevicePort for MacVirtualMicDevice {
    fn status(&self) -> VirtualMicStatus {
        current_driver_status()
    }

    fn watch(&self, on_change: Box<dyn Fn(VirtualMicStatus) + Send + Sync>) {
        let new_ptr = Box::into_raw(Box::new(WatchData { on_change }));
        let address = hardware_devices_address();

        if let Ok(mut guard) = self.watch_ptr.lock() {
            if let Some(old_ptr) = guard.take() {
                unsafe {
                    AudioObjectRemovePropertyListener(
                        kAudioObjectSystemObject,
                        &address,
                        Some(device_list_listener),
                        old_ptr as *mut c_void,
                    );
                    drop(Box::from_raw(old_ptr));
                }
            }

            unsafe {
                AudioObjectAddPropertyListener(
                    kAudioObjectSystemObject,
                    &address,
                    Some(device_list_listener),
                    new_ptr as *mut c_void,
                );
            }

            *guard = Some(new_ptr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::classify_driver_version;
    use crate::application::VirtualMicStatus;

    #[test]
    fn none_is_not_installed() {
        assert_eq!(
            classify_driver_version(None, "1.0"),
            VirtualMicStatus::NotInstalled
        );
    }

    #[test]
    fn matching_version_is_installed() {
        assert_eq!(
            classify_driver_version(Some("1.0".to_string()), "1.0"),
            VirtualMicStatus::Installed
        );
    }

    #[test]
    fn mismatched_version_is_incompatible() {
        assert_eq!(
            classify_driver_version(Some("2.0".to_string()), "1.0"),
            VirtualMicStatus::IncompatibleVersion
        );
    }

    #[test]
    fn empty_string_version_is_incompatible() {
        assert_eq!(
            classify_driver_version(Some(String::new()), "1.0"),
            VirtualMicStatus::IncompatibleVersion
        );
    }
}
