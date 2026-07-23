use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct RuntimePause {
    paused: AtomicBool,
}

impl RuntimePause {
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    pub fn toggle(&self) -> bool {
        let was_paused = self.paused.fetch_xor(true, Ordering::AcqRel);
        !was_paused
    }

    pub fn run_polling<T>(&self, automatic_toolbar: bool, action: impl FnOnce() -> T) -> Option<T> {
        (automatic_toolbar && !self.is_paused()).then(action)
    }

    pub fn run_ax_observer<T>(&self, action: impl FnOnce() -> T) -> Option<T> {
        (!self.is_paused()).then(action)
    }

    pub fn run_global_shortcut<T>(&self, action: impl FnOnce() -> T) -> Option<T> {
        (!self.is_paused()).then(action)
    }

    pub fn run_clipboard_fallback<T>(&self, action: impl FnOnce() -> T) -> Option<T> {
        (!self.is_paused()).then(action)
    }
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
}
