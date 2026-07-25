use std::collections::HashMap;
use uuid::Uuid;

struct Entry<T> {
    value: T,
    expires_at: u64,
    sequence: u64,
}

pub(super) struct CausalRegistry<T> {
    entries: HashMap<Uuid, Entry<T>>,
    capacity: usize,
    ttl_ms: u64,
    sequence: u64,
}

impl<T> CausalRegistry<T> {
    pub(super) fn new(capacity: usize, ttl_ms: u64) -> Self {
        Self {
            entries: HashMap::new(),
            capacity: capacity.max(1),
            ttl_ms,
            sequence: 0,
        }
    }

    pub(super) fn insert(&mut self, id: Uuid, value: T, now_ms: u64) {
        self.prune(now_ms);
        while self.entries.len() >= self.capacity && !self.entries.contains_key(&id) {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.sequence)
                .map(|(id, _)| *id);
            if let Some(oldest) = oldest {
                self.entries.remove(&oldest);
            }
        }
        self.sequence = self.sequence.wrapping_add(1);
        self.entries.insert(
            id,
            Entry {
                value,
                expires_at: now_ms.saturating_add(self.ttl_ms),
                sequence: self.sequence,
            },
        );
    }

    pub(super) fn get(&mut self, id: Uuid, now_ms: u64) -> Option<&T> {
        self.prune(now_ms);
        self.entries.get(&id).map(|entry| &entry.value)
    }

    pub(super) fn get_mut(&mut self, id: Uuid, now_ms: u64) -> Option<&mut T> {
        self.prune(now_ms);
        self.entries.get_mut(&id).map(|entry| &mut entry.value)
    }

    pub(super) fn remove(&mut self, id: Uuid) -> Option<T> {
        self.entries.remove(&id).map(|entry| entry.value)
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    fn prune(&mut self, now_ms: u64) {
        self.entries.retain(|_, entry| entry.expires_at > now_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    struct Tracked(Arc<AtomicUsize>);

    impl Drop for Tracked {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn ttl_capacity_and_cleanup_are_deterministic() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut registry = CausalRegistry::new(2, 10);
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let third = Uuid::new_v4();
        registry.insert(first, Tracked(drops.clone()), 0);
        registry.insert(second, Tracked(drops.clone()), 1);
        registry.insert(third, Tracked(drops.clone()), 2);
        assert!(registry.get(first, 2).is_none());
        assert!(registry.get(second, 2).is_some());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(registry.get(second, 11).is_none());
        assert_eq!(registry.len(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 2);
        registry.remove(third);
        assert_eq!(drops.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn expiration_boundary_evicts_without_sleeping() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut registry = CausalRegistry::new(1, 10);
        let id = Uuid::new_v4();
        registry.insert(id, Tracked(drops.clone()), 7);

        assert!(registry.get(id, 16).is_some());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert!(registry.get(id, 17).is_none());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn replacement_and_registry_drop_release_each_value_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let id = Uuid::new_v4();
        {
            let mut registry = CausalRegistry::new(2, 50);
            registry.insert(id, Tracked(drops.clone()), 0);
            registry.insert(id, Tracked(drops.clone()), 1);
            assert_eq!(drops.load(Ordering::SeqCst), 1);
            assert_eq!(registry.len(), 1);
        }
        assert_eq!(drops.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn capacity_evicts_in_insertion_order_even_after_reads() {
        let mut registry = CausalRegistry::new(2, 100);
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let third = Uuid::new_v4();
        registry.insert(first, 1, 0);
        registry.insert(second, 2, 1);
        assert_eq!(registry.get(first, 2), Some(&1));
        registry.insert(third, 3, 3);

        assert!(registry.get(first, 3).is_none());
        assert_eq!(registry.get(second, 3), Some(&2));
        assert_eq!(registry.get(third, 3), Some(&3));
    }
}
