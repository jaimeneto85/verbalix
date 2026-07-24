use super::*;

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

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn selected_range_has_priority_when_valid() {
    let selected = rect(10.0, 20.0, 30.0, 12.0);
    let resolved = select_geometry(
        Some(selected),
        Some(rect(1.0, 2.0, 300.0, 200.0)),
        Some(CGPoint { x: 8.0, y: 9.0 }),
    );

    assert_eq!(resolved, Some((selected, GeometrySource::SelectedRange)));
}

#[test]
fn sentinel_and_non_finite_range_fall_back_to_element_frame() {
    let frame = rect(40.0, 50.0, 600.0, 400.0);
    for invalid in [
        rect(0.0, 1117.0, 1.0, 1.0),
        rect(50.0, 60.0, 0.5, 18.0),
        rect(f64::NAN, 2.0, 3.0, 4.0),
        rect(1.0, 2.0, 0.0, 4.0),
    ] {
        assert_eq!(
            select_geometry(Some(invalid), Some(frame), None),
            Some((frame, GeometrySource::FocusedElement))
        );
    }
}

#[test]
fn cursor_is_rejected_when_element_frame_is_invalid() {
    let resolved = select_geometry(
        Some(rect(0.0, 0.0, 1.0, 1.0)),
        Some(rect(0.0, 0.0, -1.0, 10.0)),
        Some(CGPoint { x: 420.0, y: 240.0 }),
    );

    assert_eq!(resolved, None);
}

#[test]
fn negative_global_coordinates_remain_valid_for_secondary_displays() {
    let selected = rect(-1440.0, -120.0, 80.0, 18.0);
    let cursor = CGPoint {
        x: -800.0,
        y: 420.0,
    };

    assert_eq!(
        select_geometry(Some(selected), None, Some(cursor)),
        Some((selected, GeometrySource::SelectedRange))
    );
    assert_eq!(select_geometry(None, None, Some(cursor)), None);
}

#[test]
fn invalid_cursor_does_not_create_a_sentinel_rectangle() {
    assert_eq!(
        select_geometry(
            Some(rect(0.0, 0.0, 1.0, 1.0)),
            Some(rect(10.0, 10.0, f64::INFINITY, 20.0)),
            Some(CGPoint {
                x: f64::NAN,
                y: 240.0,
            }),
        ),
        None
    );
}
