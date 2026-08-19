use super::*;

fn config_with(min_voiced: u32, silence_close: u32, max_frames: u32) -> EndpointerConfig {
    EndpointerConfig {
        voice_threshold: 0.5,
        min_voiced_frames: min_voiced,
        silence_close_frames: silence_close,
        max_frames,
    }
}

#[test]
fn noise_below_threshold_stays_idle() {
    let mut ep = Endpointer::new(config_with(3, 5, 100));
    for _ in 0..10 {
        let event = ep.push_frame(0.1);
        assert!(event.is_none());
    }
    assert!(!ep.is_open());
}

#[test]
fn voiced_frames_below_minimum_drops_segment() {
    let mut ep = Endpointer::new(config_with(5, 3, 100));
    assert!(ep.push_frame(1.0).is_none());
    assert!(ep.push_frame(1.0).is_none());
    let drop_event = ep.push_frame(0.0);
    assert!(matches!(drop_event, Some(EndpointEvent::Dropped)));
    assert!(!ep.is_open());

    ep.push_frame(1.0);
    ep.push_frame(1.0);
    ep.push_frame(1.0);
    let event = ep.push_frame(1.0);
    assert!(event.is_none());
    let event = ep.push_frame(1.0);
    assert!(matches!(event, Some(EndpointEvent::Opened)));
}

#[test]
fn voiced_then_silence_closes_segment() {
    let mut ep = Endpointer::new(config_with(3, 4, 200));

    for _ in 0..3 {
        ep.push_frame(1.0);
    }

    let open_event = ep.push_frame(0.0);
    assert!(open_event.is_none());

    for _ in 0..3 {
        ep.push_frame(0.0);
    }

    assert!(ep.is_open());

    let close_event = ep.push_frame(0.0);
    assert!(matches!(close_event, Some(EndpointEvent::Closed)));
    assert!(!ep.is_open());
}

#[test]
fn max_duration_forces_close() {
    let mut ep = Endpointer::new(config_with(3, 100, 10));

    for _ in 0..3 {
        ep.push_frame(1.0);
    }

    let mut max_event = None;
    for _ in 0..7 {
        let evt = ep.push_frame(1.0);
        if evt.is_some() {
            max_event = evt;
            break;
        }
    }

    assert!(matches!(max_event, Some(EndpointEvent::MaxDurationReached)));
    assert!(!ep.is_open());
}

#[test]
fn reset_clears_state() {
    let mut ep = Endpointer::new(config_with(2, 5, 100));
    ep.push_frame(1.0);
    ep.push_frame(1.0);
    assert!(ep.is_open());

    ep.reset();
    assert!(!ep.is_open());

    for _ in 0..10 {
        ep.push_frame(0.0);
    }
    assert!(!ep.is_open());
}
