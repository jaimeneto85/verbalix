use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const GRACE_MS: u64 = 400;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0) as u64
}

pub struct ActionGuard {
    in_flight: Arc<AtomicUsize>,
    grace_deadline_ms: Arc<AtomicU64>,
    clock_ms: fn() -> u64,
}

impl Drop for ActionGuard {
    fn drop(&mut self) {
        self.grace_deadline_ms
            .store((self.clock_ms)() + GRACE_MS, Ordering::Release);
        self.in_flight.fetch_sub(1, Ordering::AcqRel);
    }
}

pub struct RuntimePause {
    paused: AtomicBool,
    in_flight: Arc<AtomicUsize>,
    grace_deadline_ms: Arc<AtomicU64>,
    clock_ms: fn() -> u64,
}

impl Default for RuntimePause {
    fn default() -> Self {
        Self {
            paused: AtomicBool::new(false),
            in_flight: Arc::new(AtomicUsize::new(0)),
            grace_deadline_ms: Arc::new(AtomicU64::new(0)),
            clock_ms: now_ms,
        }
    }
}

impl RuntimePause {
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    pub fn toggle(&self) -> bool {
        let was_paused = self.paused.fetch_xor(true, Ordering::AcqRel);
        !was_paused
    }

    pub fn is_action_in_flight(&self) -> bool {
        if self.in_flight.load(Ordering::Acquire) > 0 {
            return true;
        }
        (self.clock_ms)() < self.grace_deadline_ms.load(Ordering::Acquire)
    }

    pub fn begin_action(&self) -> ActionGuard {
        self.in_flight.fetch_add(1, Ordering::AcqRel);
        ActionGuard {
            in_flight: self.in_flight.clone(),
            grace_deadline_ms: self.grace_deadline_ms.clone(),
            clock_ms: self.clock_ms,
        }
    }

    pub fn run_polling<T>(&self, automatic_toolbar: bool, action: impl FnOnce() -> T) -> Option<T> {
        (automatic_toolbar && !self.is_paused() && !self.is_action_in_flight()).then(action)
    }

    pub fn run_ax_observer<T>(&self, action: impl FnOnce() -> T) -> Option<T> {
        (!self.is_paused() && !self.is_action_in_flight()).then(action)
    }

    pub fn run_mouse_dismiss<T>(&self, action: impl FnOnce() -> T) -> Option<T> {
        (!self.is_action_in_flight()).then(action)
    }

    pub fn run_global_shortcut<T>(&self, action: impl FnOnce() -> T) -> Option<T> {
        (!self.is_paused()).then(action)
    }

    pub fn run_clipboard_fallback<T>(&self, action: impl FnOnce() -> T) -> Option<T> {
        (!self.is_paused()).then(action)
    }
}

#[cfg(test)]
impl RuntimePause {
    #[allow(dead_code)]
    pub(crate) fn with_clock(clock_ms: fn() -> u64) -> Self {
        Self {
            paused: AtomicBool::new(false),
            in_flight: Arc::new(AtomicUsize::new(0)),
            grace_deadline_ms: Arc::new(AtomicU64::new(0)),
            clock_ms,
        }
    }
}

#[cfg(test)]
thread_local! {
    static FAKE_CLOCK_MS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn test_clock_ms() -> u64 {
    FAKE_CLOCK_MS.with(|t| t.get())
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn set_test_clock_ms(ms: u64) {
    FAKE_CLOCK_MS.with(|t| t.set(ms));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn increments(calls: &AtomicUsize) -> impl FnOnce() + '_ {
        || {
            calls.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn pause_blocks_polling_and_ax_observer_callbacks() {
        let pause = RuntimePause::default();
        let calls = AtomicUsize::new(0);

        assert!(pause.run_polling(true, increments(&calls)).is_some());
        assert!(pause.run_ax_observer(increments(&calls)).is_some());
        assert!(pause.toggle());
        assert!(pause.run_polling(true, increments(&calls)).is_none());
        assert!(pause.run_ax_observer(increments(&calls)).is_none());

        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn pause_blocks_global_shortcut_and_clipboard_fallback() {
        let pause = RuntimePause::default();
        let calls = AtomicUsize::new(0);

        assert!(pause.run_global_shortcut(increments(&calls)).is_some());
        assert!(pause.run_clipboard_fallback(increments(&calls)).is_some());
        assert!(pause.toggle());
        assert!(pause.run_global_shortcut(increments(&calls)).is_none());
        assert!(pause.run_clipboard_fallback(increments(&calls)).is_none());

        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn resume_reenables_every_runtime_entry_point() {
        let pause = RuntimePause::default();
        assert!(pause.toggle());
        assert!(!pause.toggle());

        assert!(pause.run_polling(true, || ()).is_some());
        assert!(pause.run_ax_observer(|| ()).is_some());
        assert!(pause.run_global_shortcut(|| ()).is_some());
        assert!(pause.run_clipboard_fallback(|| ()).is_some());
    }

    #[test]
    fn polling_stays_disabled_when_automatic_toolbar_is_off() {
        let pause = RuntimePause::default();

        assert!(pause.run_polling(false, || ()).is_none());
    }

    #[test]
    fn in_flight_guard_suppresses_all_detection_entry_points() {
        set_test_clock_ms(0);
        let pause = RuntimePause::with_clock(test_clock_ms);
        let _guard = pause.begin_action();

        assert!(pause.run_polling(true, || ()).is_none());
        assert!(pause.run_ax_observer(|| ()).is_none());
        assert!(pause.run_mouse_dismiss(|| ()).is_none());
    }

    #[test]
    fn in_flight_guard_suppresses_during_grace_after_drop() {
        set_test_clock_ms(1000);
        let pause = RuntimePause::with_clock(test_clock_ms);
        let guard = pause.begin_action();
        drop(guard);

        assert!(pause.is_action_in_flight());
        assert!(pause.run_polling(true, || ()).is_none());
        assert!(pause.run_ax_observer(|| ()).is_none());
        assert!(pause.run_mouse_dismiss(|| ()).is_none());
    }

    #[test]
    fn in_flight_reentrancy_requires_all_guards_dropped_and_grace_expired() {
        set_test_clock_ms(1000);
        let pause = RuntimePause::with_clock(test_clock_ms);
        let g1 = pause.begin_action();
        let g2 = pause.begin_action();
        drop(g1);

        assert!(pause.is_action_in_flight());

        drop(g2);

        assert!(pause.is_action_in_flight());

        set_test_clock_ms(1000 + GRACE_MS + 1);

        assert!(!pause.is_action_in_flight());
        assert!(pause.run_polling(true, || ()).is_some());
        assert!(pause.run_ax_observer(|| ()).is_some());
        assert!(pause.run_mouse_dismiss(|| ()).is_some());
    }

    #[test]
    fn grace_period_expires_at_deadline_boundary() {
        set_test_clock_ms(500);
        let pause = RuntimePause::with_clock(test_clock_ms);
        let g = pause.begin_action();
        drop(g);

        assert!(pause.is_action_in_flight());

        set_test_clock_ms(500 + GRACE_MS - 1);
        assert!(pause.is_action_in_flight());

        set_test_clock_ms(500 + GRACE_MS);
        assert!(!pause.is_action_in_flight());

        set_test_clock_ms(500 + GRACE_MS + 1);
        assert!(!pause.is_action_in_flight());
    }
}
