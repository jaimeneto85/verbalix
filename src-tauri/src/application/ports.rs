use crate::{
    application::{MutationProjection, MutationReceipt, PublicationGuard},
    domain::{
        EnrollmentSample, MicrophonePermission, Rect, SelectionSnapshot, VerbalixError,
        VoiceProfileView,
    },
};
use serde::{Deserialize, Serialize};

pub trait SelectionPort: Send + Sync {
    fn permission_granted(&self, prompt: bool) -> bool;
    fn capture(&self) -> Result<SelectionSnapshot, VerbalixError>;
    fn replace(&self, expected: &SelectionSnapshot, text: &str) -> Result<(), VerbalixError>;
    fn replace_guarded(
        &self,
        expected: &SelectionSnapshot,
        text: &str,
        lease: &PublicationGuard,
    ) -> Result<MutationReceipt, VerbalixError> {
        if !lease.try_claim_write() {
            return Err(VerbalixError::StaleSelection);
        }
        self.replace(expected, text)?;
        Ok(MutationReceipt {
            id: uuid::Uuid::new_v4(),
            snapshot_id: expected.id,
            request_id: lease.request_id(),
        })
    }
    fn replace_guarded_with_id(
        &self,
        expected: &SelectionSnapshot,
        text: &str,
        lease: &PublicationGuard,
        _mutation_id: uuid::Uuid,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.replace_guarded(expected, text, lease)
    }
    fn restore_guarded(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
        lease: &PublicationGuard,
    ) -> Result<MutationReceipt, VerbalixError> {
        if !lease.try_claim_write() {
            return Err(VerbalixError::StaleSelection);
        }
        self.restore(expected, transformed_text)?;
        Ok(MutationReceipt {
            id: uuid::Uuid::new_v4(),
            snapshot_id: expected.id,
            request_id: lease.request_id(),
        })
    }
    fn restore_guarded_with_id(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
        lease: &PublicationGuard,
        _mutation_id: uuid::Uuid,
    ) -> Result<MutationReceipt, VerbalixError> {
        self.restore_guarded(expected, transformed_text, lease)
    }
    fn restore(
        &self,
        expected: &SelectionSnapshot,
        transformed_text: &str,
    ) -> Result<(), VerbalixError>;

    fn discard_snapshot(&self, _snapshot_id: uuid::Uuid) {}

    fn reconcile_mutation(
        &self,
        _mutation_id: uuid::Uuid,
    ) -> Result<Option<MutationProjection>, VerbalixError> {
        Ok(None)
    }
}

pub trait OverlayPort: Send + Sync {
    fn show_toolbar(&self, bounds: Rect) -> Result<(), VerbalixError>;
    fn show_toolbar_guarded(
        &self,
        bounds: Rect,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        if guard.may_publish() {
            self.show_toolbar(bounds)
        } else {
            Ok(())
        }
    }
    fn show_note(&self, bounds: Rect, text: &str) -> Result<(), VerbalixError>;
    fn show_preview(
        &self,
        bounds: Rect,
        request_id: uuid::Uuid,
        text: &str,
    ) -> Result<(), VerbalixError>;
    fn show_undo(&self, bounds: Rect, text: &str) -> Result<(), VerbalixError>;
    fn show_note_guarded(
        &self,
        bounds: Rect,
        text: &str,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        if guard.may_publish() {
            self.show_note(bounds, text)
        } else {
            Ok(())
        }
    }
    fn show_preview_guarded(
        &self,
        bounds: Rect,
        request_id: uuid::Uuid,
        text: &str,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        if guard.may_publish() {
            self.show_preview(bounds, request_id, text)
        } else {
            Ok(())
        }
    }
    fn show_undo_guarded(
        &self,
        bounds: Rect,
        text: &str,
        guard: PublicationGuard,
    ) -> Result<(), VerbalixError> {
        if guard.may_publish() {
            self.show_undo(bounds, text)
        } else {
            Ok(())
        }
    }
    fn hide_all(&self) -> Result<(), VerbalixError>;
}

pub trait ClipboardPort: Send + Sync {
    fn read_text(&self) -> Result<Option<String>, VerbalixError>;
}

pub trait AudioCapturePort: Send + Sync {
    fn start(&self) -> Result<(), VerbalixError>;
    fn stop(&self) -> Result<EnrollmentSample, VerbalixError>;
    fn cancel(&self);
    fn level(&self) -> f32;
    fn permission_status(&self) -> MicrophonePermission;
}

pub trait AudioStreamPort: Send + Sync {
    fn start_stream(
        &self,
        sink: Box<dyn Fn(Vec<f32>, u32, u16) + Send + Sync + 'static>,
        error_sink: Box<dyn Fn() + Send + Sync + 'static>,
    ) -> Result<(), crate::domain::VerbalixError>;

    fn stop_stream(&self);
}

pub trait VoicePipelinePort: Send + Sync {
    fn interpret<'a>(
        &'a self,
        session_id: crate::domain::LiveSessionId,
        segment_id: crate::domain::SegmentId,
        wav_bytes: Vec<u8>,
        target_language: &'a str,
        token: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = crate::domain::InterpretOutcome> + Send + 'a>,
    >;

    fn interpret_stream<'a>(
        &'a self,
        session_id: crate::domain::LiveSessionId,
        segment_id: crate::domain::SegmentId,
        wav_bytes: Vec<u8>,
        target_language: &'a str,
        token: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        crate::application::streaming_audio::StreamSegmentHandle,
                        crate::domain::InterpretOutcome,
                    >,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let outcome = self
                .interpret(session_id, segment_id, wav_bytes, target_language, token)
                .await;
            Err(outcome)
        })
    }
}

pub trait AudioPreviewPort: Send + Sync {
    fn play(&self, wav_bytes: Vec<u8>) -> Result<(), crate::domain::VerbalixError>;
    fn stop(&self);
    fn play_stream(
        &self,
        handle: crate::application::streaming_audio::StreamSegmentHandle,
    ) -> Result<(), crate::domain::VerbalixError> {
        use std::sync::atomic::Ordering;
        while !handle.cancel.load(Ordering::Relaxed) && !handle.complete.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VirtualMicStatus {
    NotInstalled,
    Installed,
    IncompatibleVersion,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualMicMetrics {
    pub buffer_depth: u32,
    pub underruns: u64,
}

pub trait VirtualMicDevicePort: Send + Sync {
    fn status(&self) -> VirtualMicStatus;
    fn watch(&self, on_change: Box<dyn Fn(VirtualMicStatus) + Send + Sync>);
}

pub trait VirtualMicOutputPort: Send + Sync {
    fn open(&self) -> Result<(), VerbalixError>;
    fn enqueue(&self, samples_48k: Vec<f32>, channels: u16);
    fn close(&self);
    fn metrics(&self) -> VirtualMicMetrics;
}

pub trait VoiceEnrollmentPort: Send + Sync {
    fn enroll<'a>(
        &'a self,
        sample: &'a EnrollmentSample,
        display_name: &'a str,
        request_id: uuid::Uuid,
        token: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<VoiceProfileView, VerbalixError>> + Send + 'a>,
    >;

    fn delete<'a>(
        &'a self,
        id: uuid::Uuid,
        token: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), VerbalixError>> + Send + 'a>>;

    fn status<'a>(
        &'a self,
        id: uuid::Uuid,
        token: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Option<VoiceProfileView>, VerbalixError>>
                + Send
                + 'a,
        >,
    >;
}
