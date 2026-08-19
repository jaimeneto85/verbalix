use super::*;
use crate::domain::{InterpretOutcome, LiveSessionId, SegmentId, VerbalixError};

fn outcome(seg: u64, ok: bool) -> InterpretOutcome {
    let session_id = LiveSessionId(uuid::Uuid::nil());
    let segment_id = SegmentId(seg);
    if ok {
        InterpretOutcome {
            session_id,
            segment_id,
            result: Ok(crate::domain::SegmentResult {
                audio_base64: String::new(),
                detected_language: "en".to_owned(),
                stage_ms: crate::domain::StageDurations {
                    stt: 0,
                    translate: 0,
                    tts: 0,
                },
            }),
        }
    } else {
        InterpretOutcome {
            session_id,
            segment_id,
            result: Err(VerbalixError::InterpretationFailed),
        }
    }
}

fn ready_ids(events: &[QueueEvent]) -> Vec<u64> {
    events
        .iter()
        .filter_map(|e| {
            if let QueueEvent::Ready(o) = e {
                Some(o.segment_id.0)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn in_order_delivery() {
    let mut q = LiveQueue::new(8);
    let events = q.insert(outcome(0, true));
    assert_eq!(ready_ids(&events), vec![0]);

    let events = q.insert(outcome(1, true));
    assert_eq!(ready_ids(&events), vec![1]);
}

#[test]
fn out_of_order_buffered_and_delivered_when_predecessor_arrives() {
    let mut q = LiveQueue::new(8);

    let events = q.insert(outcome(1, true));
    assert!(ready_ids(&events).is_empty());

    let events = q.insert(outcome(2, true));
    assert!(ready_ids(&events).is_empty());

    let events = q.insert(outcome(0, true));
    assert_eq!(ready_ids(&events), vec![0, 1, 2]);
}

#[test]
fn failed_segment_unblocks_successor() {
    let mut q = LiveQueue::new(8);

    let events = q.insert(outcome(1, true));
    assert!(ready_ids(&events).is_empty());

    let events = q.insert(outcome(0, false));
    assert_eq!(ready_ids(&events), vec![0, 1]);
}

#[test]
fn at_capacity_drops_oldest_when_new_arrives() {
    let mut q = LiveQueue::new(2);

    q.insert(outcome(2, true));
    q.insert(outcome(3, true));

    let events = q.insert(outcome(4, true));

    let dropped: Vec<u64> = events
        .iter()
        .filter_map(|e| {
            if let QueueEvent::Dropped { segment_id } = e {
                Some(segment_id.0)
            } else {
                None
            }
        })
        .collect();

    assert!(dropped.contains(&2));
}

#[test]
fn reset_clears_queue_and_resets_pointer() {
    let mut q = LiveQueue::new(8);
    q.insert(outcome(1, true));
    q.insert(outcome(2, true));

    q.reset(SegmentId(0));

    let events = q.insert(outcome(0, true));
    assert_eq!(ready_ids(&events), vec![0]);
}
