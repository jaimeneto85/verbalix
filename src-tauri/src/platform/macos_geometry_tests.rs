use super::*;

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn point(x: f64, y: f64) -> CGPoint {
    CGPoint { x, y }
}

fn cursor_rect(cursor: CGPoint) -> Rect {
    rect(cursor.x, cursor.y, 1.0, 1.0)
}

fn assert_cursor(frame: Rect, cursor: CGPoint) {
    assert_eq!(
        select_geometry(None, Some(frame), Some(cursor)),
        Some((cursor_rect(cursor), GeometrySource::Cursor))
    );
}

fn assert_frame(frame: Rect, cursor: Option<CGPoint>) {
    assert_eq!(
        select_geometry(None, Some(frame), cursor),
        Some((frame, GeometrySource::FocusedElement))
    );
}

#[test]
fn geometry_decoding_requires_the_expected_ax_value_type() {
    assert!(ax_value_type_matches(
        true,
        AX_VALUE_CG_RECT,
        AX_VALUE_CG_RECT
    ));
    assert!(!ax_value_type_matches(
        false,
        AX_VALUE_CG_RECT,
        AX_VALUE_CG_RECT
    ));
    assert!(!ax_value_type_matches(
        true,
        AX_VALUE_CG_POINT,
        AX_VALUE_CG_RECT
    ));
    assert!(!ax_value_type_matches(
        true,
        AX_VALUE_CG_SIZE,
        AX_VALUE_CG_RECT
    ));
}

#[test]
fn selected_range_wins_over_every_other_geometry() {
    let selected = rect(-80.0, 20.0, 30.0, 12.0);
    let frames = [
        Some(rect(-100.0, 0.0, 300.0, 200.0)),
        Some(rect(f64::NAN, 0.0, 300.0, 200.0)),
        None,
    ];
    let cursors = [
        Some(point(-75.0, 25.0)),
        Some(point(f64::INFINITY, f64::NEG_INFINITY)),
        None,
    ];

    for frame in frames {
        for cursor in cursors {
            assert_eq!(
                select_geometry(Some(selected), frame, cursor),
                Some((selected, GeometrySource::SelectedRange))
            );
        }
    }
}

#[test]
fn invalid_selected_ranges_continue_through_the_fallback_chain() {
    let frame = rect(40.0, 50.0, 600.0, 400.0);
    let cursor = point(100.0, 120.0);
    let invalid_ranges = [
        rect(0.0, 1117.0, 1.0, 1.0),
        rect(50.0, 60.0, 1.0, 18.0),
        rect(50.0, 60.0, 18.0, 1.0),
        rect(50.0, 60.0, 0.0, 18.0),
        rect(50.0, 60.0, -1.0, 18.0),
        rect(f64::NAN, 60.0, 18.0, 18.0),
        rect(50.0, f64::INFINITY, 18.0, 18.0),
        rect(50.0, 60.0, f64::NEG_INFINITY, 18.0),
        rect(50.0, 60.0, 18.0, f64::NAN),
    ];

    for invalid in invalid_ranges {
        assert_eq!(
            select_geometry(Some(invalid), Some(frame), Some(cursor)),
            Some((cursor_rect(cursor), GeometrySource::Cursor))
        );
    }
}

#[test]
fn cursor_inside_a_valid_frame_wins_over_the_frame() {
    let frame = rect(10.0, 20.0, 100.0, 80.0);
    assert_cursor(frame, point(60.0, 50.0));
}

#[test]
fn all_frame_corners_are_inclusively_contained() {
    let frame = rect(10.0, 20.0, 100.0, 80.0);
    for cursor in [
        point(10.0, 20.0),
        point(110.0, 20.0),
        point(10.0, 100.0),
        point(110.0, 100.0),
    ] {
        assert_cursor(frame, cursor);
    }
}

#[test]
fn points_immediately_outside_each_frame_edge_fall_back_to_the_frame() {
    let frame = rect(10.0, 20.0, 100.0, 80.0);
    let delta = 1e-9;
    for cursor in [
        point(10.0 - delta, 60.0),
        point(110.0 + delta, 60.0),
        point(60.0, 20.0 - delta),
        point(60.0, 100.0 + delta),
    ] {
        assert_frame(frame, Some(cursor));
    }
}

#[test]
fn points_immediately_outside_each_corner_fall_back_to_the_frame() {
    let frame = rect(10.0, 20.0, 100.0, 80.0);
    let delta = 1e-9;
    for cursor in [
        point(10.0 - delta, 20.0 - delta),
        point(110.0 + delta, 20.0 - delta),
        point(10.0 - delta, 100.0 + delta),
        point(110.0 + delta, 100.0 + delta),
    ] {
        assert_frame(frame, Some(cursor));
    }
}

#[test]
fn non_finite_cursors_fall_back_to_a_valid_frame() {
    let frame = rect(-500.0, -300.0, 1000.0, 600.0);
    for cursor in [
        point(f64::NAN, 0.0),
        point(0.0, f64::NAN),
        point(f64::INFINITY, 0.0),
        point(0.0, f64::INFINITY),
        point(f64::NEG_INFINITY, 0.0),
        point(0.0, f64::NEG_INFINITY),
    ] {
        assert_frame(frame, Some(cursor));
    }
}

#[test]
fn absent_cursor_falls_back_to_a_valid_frame() {
    assert_frame(rect(10.0, 20.0, 100.0, 80.0), None);
}

#[test]
fn cursor_is_never_authorized_without_a_valid_frame() {
    let cursor = Some(point(0.0, 0.0));
    let invalid_frames = [
        None,
        Some(rect(0.0, 0.0, 0.0, 10.0)),
        Some(rect(0.0, 0.0, 10.0, 0.0)),
        Some(rect(0.0, 0.0, -1.0, 10.0)),
        Some(rect(0.0, 0.0, 10.0, -1.0)),
        Some(rect(f64::NAN, 0.0, 10.0, 10.0)),
        Some(rect(0.0, f64::INFINITY, 10.0, 10.0)),
        Some(rect(0.0, 0.0, f64::INFINITY, 10.0)),
        Some(rect(0.0, 0.0, 10.0, f64::NEG_INFINITY)),
    ];

    for frame in invalid_frames {
        assert_eq!(select_geometry(None, frame, cursor), None);
    }
}

#[test]
fn overflowing_upper_bounds_reject_the_cursor_and_preserve_the_frame() {
    let x_overflow = rect(f64::MAX, 0.0, f64::MAX, 10.0);
    let y_overflow = rect(0.0, f64::MAX, 10.0, f64::MAX);

    assert_frame(x_overflow, Some(point(f64::MAX, 5.0)));
    assert_frame(y_overflow, Some(point(5.0, f64::MAX)));
}

#[test]
fn negative_global_coordinates_support_secondary_displays() {
    let frame = rect(-1600.0, -1200.0, 800.0, 600.0);
    assert_cursor(frame, point(-1200.0, -900.0));
    assert_frame(frame, Some(point(-799.999999999, -900.0)));
}

#[test]
fn frames_crossing_the_global_origin_contain_points_on_both_sides() {
    let frame = rect(-200.0, -100.0, 400.0, 200.0);
    for cursor in [
        point(-199.0, -99.0),
        point(-1.0, -1.0),
        point(0.0, 0.0),
        point(199.0, 99.0),
    ] {
        assert_cursor(frame, cursor);
    }
}

#[test]
fn no_valid_geometry_returns_none() {
    assert_eq!(select_geometry(None, None, None), None);
    assert_eq!(
        select_geometry(
            Some(rect(0.0, 0.0, 1.0, 1.0)),
            None,
            Some(point(0.0, 0.0))
        ),
        None
    );
}
