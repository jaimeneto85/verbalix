use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};

const ARMED: u8 = 0;
const AUTHORIZING: u8 = 1;
const IN_SETTER: u8 = 2;
const COMMITTED: u8 = 3;
const CANCELLED: u8 = 4;

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

    pub(super) fn begin_authorizing(&self) -> bool {
        self.state
            .compare_exchange(ARMED, AUTHORIZING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(super) fn enter_setter(&self) -> bool {
        self.state
            .compare_exchange(AUTHORIZING, IN_SETTER, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(super) fn cancel_authorizing(&self) {
        let _ = self.state.compare_exchange(
            AUTHORIZING,
            CANCELLED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    pub(super) fn claim_observation(&self) -> bool {
        loop {
            let observed = self.state.load(Ordering::Acquire);
            match observed {
                ARMED | AUTHORIZING => {
                    if self
                        .state
                        .compare_exchange(observed, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
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
        let mut observed = self.state.load(Ordering::Acquire);
        loop {
            if observed == CANCELLED {
                return;
            }
            match self.state.compare_exchange(
                observed,
                CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(current) => observed = current,
            }
        }
    }
}
