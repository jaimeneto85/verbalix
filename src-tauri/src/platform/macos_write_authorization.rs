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
        self.begin_setter_with(|| {}, || {})
    }

    fn begin_setter_with(
        &self,
        after_authorizing: impl FnOnce(),
        after_epoch_valid: impl FnOnce(),
    ) -> bool {
        if !self.phase.begin_authorizing() {
            return false;
        }
        after_authorizing();
        if !self.epoch.is_current(self.expected) {
            self.phase.cancel_authorizing();
            return false;
        }
        after_epoch_valid();
        self.phase.enter_setter()
    }

    #[cfg(test)]
    pub(super) fn begin_setter_after_authorizing(&self, after_authorizing: impl FnOnce()) -> bool {
        self.begin_setter_with(after_authorizing, || {})
    }

    #[cfg(test)]
    pub(super) fn begin_setter_after_epoch_valid(&self, after_epoch_valid: impl FnOnce()) -> bool {
        self.begin_setter_with(|| {}, after_epoch_valid)
    }

    pub(super) fn commit(&self) {
        self.phase.commit();
    }
}
