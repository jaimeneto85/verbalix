use super::*;

fn config_with(min_voiced: u32, silence_close: u32, max_frames: u32) -> EndpointerConfig {
    EndpointerConfig {
        voice_threshold: 0.5,
        min_voiced_frames: min_voiced,
        silence_close_frames: silence_close,
        silence_close_frames_early: silence_close,
        min_voiced_for_early_close: u32::MAX,
        max_frames,
    }
}

fn config_with_hysteresis(
    min_voiced: u32,
    silence_close: u32,
    silence_close_early: u32,
    min_voiced_for_early: u32,
    max_frames: u32,
) -> EndpointerConfig {
    EndpointerConfig {
        voice_threshold: 0.5,
        min_voiced_frames: min_voiced,
        silence_close_frames: silence_close,
        silence_close_frames_early: silence_close_early,
        min_voiced_for_early_close: min_voiced_for_early,
        max_frames,
    }
}

fn push_voiced(ep: &mut Endpointer, count: u32) {
    for _ in 0..count {
        ep.push_frame(1.0);
    }
}

fn push_silent(ep: &mut Endpointer, count: u32) {
    for _ in 0..count {
        ep.push_frame(0.0);
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

#[test]
fn short_utterance_uses_conservative_silence_threshold() {
    let mut ep = Endpointer::new(config_with_hysteresis(3, 10, 3, 20, 500));
    push_voiced(&mut ep, 3);
    assert!(ep.is_open());

    push_silent(&mut ep, 4);
    assert!(
        ep.is_open(),
        "4 silent frames < conservative=10; segment must remain open"
    );

    push_silent(&mut ep, 7);
    assert!(
        !ep.is_open(),
        "11 total silent frames > conservative=10; must close"
    );
}

#[test]
fn long_utterance_uses_early_silence_threshold() {
    let mut ep = Endpointer::new(config_with_hysteresis(3, 10, 3, 10, 500));
    push_voiced(&mut ep, 10);
    assert!(ep.is_open());

    push_silent(&mut ep, 3);
    assert!(
        ep.is_open(),
        "3 silent frames = early threshold; not yet exceeded (> not >=)"
    );

    let close_event = ep.push_frame(0.0);
    assert!(
        matches!(close_event, Some(EndpointEvent::Closed)),
        "4th silent frame > early threshold 3; must close"
    );
    assert!(!ep.is_open());
}

#[test]
fn pause_in_middle_does_not_close_with_conservative_threshold() {
    let mut ep = Endpointer::new(config_with_hysteresis(3, 10, 3, 20, 500));

    push_voiced(&mut ep, 3);
    assert!(ep.is_open());

    push_silent(&mut ep, 5);
    assert!(
        ep.is_open(),
        "5 silences < conservative threshold 10; still open"
    );

    push_voiced(&mut ep, 5);
    assert!(ep.is_open(), "resumed voicing; segment stays open");

    push_silent(&mut ep, 5);
    assert!(
        ep.is_open(),
        "5 silences again; voiced_frames=13 < min_voiced_for_early=20; conservative active"
    );
}

#[test]
fn voiced_frames_accumulate_across_pause_until_hysteresis_activates() {
    let mut ep = Endpointer::new(config_with_hysteresis(3, 10, 3, 8, 500));

    push_voiced(&mut ep, 3);
    assert!(ep.is_open());

    push_silent(&mut ep, 5);
    assert!(
        ep.is_open(),
        "5 silent < conservative=10; voiced_frames=3 < min_voiced_for_early=8; still open"
    );

    push_voiced(&mut ep, 5);
    assert!(
        ep.is_open(),
        "voiced resumes; voiced_frames now 8 = min_voiced_for_early"
    );

    push_silent(&mut ep, 3);
    assert!(
        ep.is_open(),
        "3 silences = early threshold; not yet exceeded (> not >=)"
    );

    let close_event = ep.push_frame(0.0);
    assert!(
        matches!(close_event, Some(EndpointEvent::Closed)),
        "voiced_frames=8 >= min_voiced_for_early=8 now; 4 silent > early=3 → closes"
    );
    assert!(!ep.is_open());
}

#[test]
fn hysteresis_activates_after_sufficient_voiced_frames() {
    let mut ep = Endpointer::new(config_with_hysteresis(5, 10, 3, 10, 500));

    push_voiced(&mut ep, 5);
    assert!(ep.is_open());

    push_silent(&mut ep, 4);
    assert!(
        ep.is_open(),
        "voiced_frames=5 < min_voiced_for_early=10; conservative=10 active; 4 < 10"
    );

    ep.reset();

    push_voiced(&mut ep, 5);
    assert!(ep.is_open());
    push_voiced(&mut ep, 5);

    push_silent(&mut ep, 3);
    assert!(
        ep.is_open(),
        "3 silences = early threshold; not yet exceeded"
    );

    let close_event = ep.push_frame(0.0);
    assert!(
        matches!(close_event, Some(EndpointEvent::Closed)),
        "voiced_frames=10 >= min_voiced_for_early=10; early=3; 4 > 3 closes"
    );
}
