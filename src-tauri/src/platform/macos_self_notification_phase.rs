use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};

const ARMED: u8 = 0;
const IN_SETTER: u8 = 1;
const COMMITTED: u8 = 2;
const CANCELLED: u8 = 3;

#[derive(Clone)]
pub(super) struct SelfNotificationPhase {
    state: Arc<AtomicU8>,
}

impl SelfNotificationPhase {
    pub(super) fn armed() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(ARMED)),
        }
    }

    pub(super) fn begin_setter(&self) -> bool {
        self.state
            .compare_exchange(ARMED, IN_SETTER, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(super) fn claim_observation(&self) -> bool {
        loop {
            match self.state.load(Ordering::Acquire) {
                ARMED => {
                    if self
                        .state
                        .compare_exchange(ARMED, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        return false;
                    }
                }
                IN_SETTER | COMMITTED => return true,
                CANCELLED => return false,
                _ => return false,
            }
        }
    }

    pub(super) fn commit(&self) {
        let _ =
            self.state
                .compare_exchange(IN_SETTER, COMMITTED, Ordering::AcqRel, Ordering::Acquire);
    }

    pub(super) fn cancel(&self) {
        self.state.store(CANCELLED, Ordering::Release);
    }
}
