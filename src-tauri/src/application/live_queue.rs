use crate::domain::{InterpretOutcome, SegmentId};
use std::collections::BTreeMap;

pub enum QueueEvent {
    Ready(InterpretOutcome),
    Dropped { segment_id: SegmentId },
}

pub struct LiveQueue {
    capacity: usize,
    next_to_emit: SegmentId,
    pending: BTreeMap<SegmentId, InterpretOutcome>,
}

impl LiveQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            next_to_emit: SegmentId(0),
            pending: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, outcome: InterpretOutcome) -> Vec<QueueEvent> {
        let mut events = Vec::new();

        if self.pending.len() >= self.capacity {
            if let Some(oldest_key) = self.pending.keys().next().copied() {
                self.pending.remove(&oldest_key);
                events.push(QueueEvent::Dropped {
                    segment_id: oldest_key,
                });
            }
        }

        self.pending.insert(outcome.segment_id, outcome);

        while let Some(&key) = self.pending.keys().next() {
            if key == self.next_to_emit {
                let out = self.pending.remove(&key).unwrap();
                self.next_to_emit = self.next_to_emit.next();
                events.push(QueueEvent::Ready(out));
            } else {
                break;
            }
        }

        events
    }

    pub fn drain_all(&mut self) -> Vec<InterpretOutcome> {
        std::mem::take(&mut self.pending).into_values().collect()
    }

    pub fn reset(&mut self, next_segment: SegmentId) {
        self.pending.clear();
        self.next_to_emit = next_segment;
    }
}

#[cfg(test)]
#[path = "live_queue_tests.rs"]
mod tests;
