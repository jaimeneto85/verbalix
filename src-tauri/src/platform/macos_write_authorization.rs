use super::{causal_epoch::CausalEpoch, macos_self_notification_phase::SelfNotificationPhase};

pub(super) struct AxWriteAuthorization {
    epoch: CausalEpoch,
    expected: u64,
    phase: SelfNotificationPhase,
}

impl AxWriteAuthorization {
    pub(super) fn new(epoch: CausalEpoch, expected: u64, phase: SelfNotificationPhase) -> Self {
        Self {
            epoch,
            expected,
            phase,
        }
    }

    pub(super) fn begin_setter(&self) -> bool {
        self.phase.begin_setter() && self.epoch.is_current(self.expected)
    }

    pub(super) fn commit(&self) {
        self.phase.commit();
    }
}
