use super::causal_epoch::CausalEpoch;

pub(super) struct AxWriteAuthorization {
    epoch: CausalEpoch,
    expected: u64,
}

impl AxWriteAuthorization {
    pub(super) fn new(epoch: CausalEpoch, expected: u64) -> Self {
        Self { epoch, expected }
    }

    pub(super) fn is_current(&self) -> bool {
        self.epoch.is_current(self.expected)
    }
}
