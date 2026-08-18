use crate::domain::MicrophonePermission;
use std::sync::{Arc, Mutex};

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

pub fn microphone_permission_status() -> MicrophonePermission {
    unsafe {
        use objc2_foundation::NSString;
        let cls = objc2::class!(AVCaptureDevice);
        let media_audio = NSString::from_str("soun");
        let status: i64 = objc2::msg_send![cls, authorizationStatusForMediaType: &*media_audio];
        match status {
            0 => MicrophonePermission::NotDetermined,
            1 => MicrophonePermission::Restricted,
            2 => MicrophonePermission::Denied,
            3 => MicrophonePermission::Authorized,
            _ => MicrophonePermission::Denied,
        }
    }
}

pub fn request_microphone_permission<F: FnOnce(MicrophonePermission) + Send + 'static>(
    callback: F,
) {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::NSString;

    let cb = Arc::new(Mutex::new(Some(callback)));
    let cb_clone = Arc::clone(&cb);

    let block = RcBlock::new(move |granted: Bool| {
        if let Some(f) = cb_clone.lock().ok().and_then(|mut g| g.take()) {
            f(if granted.as_bool() {
                MicrophonePermission::Authorized
            } else {
                MicrophonePermission::Denied
            });
        }
    });

    unsafe {
        let cls = objc2::class!(AVCaptureDevice);
        let media_audio = NSString::from_str("soun");
        let _: () = objc2::msg_send![
            cls,
            requestAccessForMediaType: &*media_audio,
            completionHandler: &*block
        ];
    }
}
