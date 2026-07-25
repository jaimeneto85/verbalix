use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

#[derive(Clone, Default)]
pub(super) struct CausalEpoch {
    value: Arc<AtomicU64>,
}

impl CausalEpoch {
    pub(super) fn current(&self) -> u64 {
        self.value.load(Ordering::Acquire)
    }

    pub(super) fn bump(&self) {
        self.value.fetch_add(1, Ordering::AcqRel);
    }

    pub(super) fn is_current(&self, expected: u64) -> bool {
        self.current() == expected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_signal_without_a_fifo_or_lock() {
        let epoch = CausalEpoch::default();
        let signal = epoch.clone();
        let captured = epoch.current();
        signal.bump();
        assert!(!epoch.is_current(captured));
    }
}
