use crate::{
    application::{VirtualMicMetrics, VirtualMicStatus},
    domain::VerbalixError,
    AppRuntime,
};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VirtualMicStatusResponse {
    status: VirtualMicStatus,
}

#[tauri::command]
pub(crate) fn virtual_mic_status(
    runtime: State<'_, Arc<AppRuntime>>,
) -> Result<VirtualMicStatusResponse, VerbalixError> {
    let status = runtime.virtual_mic_status();
    let VirtualMicMetrics {
        buffer_depth,
        underruns,
    } = runtime.live_coordinator.vmic_metrics();
    crate::diagnostics::virtual_mic(status, buffer_depth, underruns);
    Ok(VirtualMicStatusResponse { status })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_variants_serialize_as_camel_case_strings() {
        let not_installed = VirtualMicStatusResponse {
            status: VirtualMicStatus::NotInstalled,
        };
        assert_eq!(
            serde_json::to_string(&not_installed).unwrap(),
            r#"{"status":"notInstalled"}"#
        );

        let installed = VirtualMicStatusResponse {
            status: VirtualMicStatus::Installed,
        };
        assert_eq!(
            serde_json::to_string(&installed).unwrap(),
            r#"{"status":"installed"}"#
        );

        let incompatible = VirtualMicStatusResponse {
            status: VirtualMicStatus::IncompatibleVersion,
        };
        assert_eq!(
            serde_json::to_string(&incompatible).unwrap(),
            r#"{"status":"incompatibleVersion"}"#
        );
    }
}
