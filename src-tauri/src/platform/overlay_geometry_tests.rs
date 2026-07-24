use super::*;

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn screen(full: Rect, visible: Rect) -> ScreenFrame {
    ScreenFrame {
        full: CocoaRect(full),
        visible: CocoaRect(visible),
    }
}

fn primary() -> ScreenFrame {
    screen(rect(0.0, 0.0, 1000.0, 900.0), rect(0.0, 0.0, 1000.0, 800.0))
}

#[test]
fn converts_ax_and_cocoa_with_a_nonzero_main_origin() {
    let ax = AxRect(rect(-450.0, 120.0, 80.0, 24.0));

    let cocoa = ax_to_cocoa(ax, 980.0).unwrap();

    assert_eq!(cocoa, CocoaRect(rect(-450.0, 836.0, 80.0, 24.0)));
    assert_eq!(cocoa_to_ax(cocoa, 980.0), Some(ax));
}

#[test]
fn centers_above_without_clamping() {
    let origin = anchored_origin(
        CocoaRect(rect(200.0, 300.0, 100.0, 20.0)),
        236.0,
        52.0,
        primary(),
    );

    assert_eq!(origin, Some(CocoaPoint { x: 132.0, y: 330.0 }));
}

#[test]
fn falls_below_when_above_does_not_fit() {
    let origin = anchored_origin(
        CocoaRect(rect(200.0, 740.0, 100.0, 20.0)),
        236.0,
        52.0,
        primary(),
    );

    assert_eq!(origin, Some(CocoaPoint { x: 132.0, y: 678.0 }));
}

#[test]
fn clamps_to_all_visible_edges() {
    let cases = [
        (
            rect(-80.0, 300.0, 20.0, 20.0),
            CocoaPoint { x: 8.0, y: 330.0 },
        ),
        (
            rect(990.0, 300.0, 20.0, 20.0),
            CocoaPoint { x: 756.0, y: 330.0 },
        ),
        (
            rect(200.0, -70.0, 100.0, 20.0),
            CocoaPoint { x: 132.0, y: 8.0 },
        ),
        (
            rect(200.0, 790.0, 100.0, 20.0),
            CocoaPoint { x: 132.0, y: 728.0 },
        ),
    ];

    for (selection, expected) in cases {
        assert_eq!(
            anchored_origin(CocoaRect(selection), 236.0, 52.0, primary()),
            Some(expected)
        );
    }
}

#[test]
fn centers_an_overlay_larger_than_the_visible_frame() {
    let tiny = screen(rect(0.0, 0.0, 100.0, 80.0), rect(0.0, 0.0, 100.0, 80.0));

    let origin = anchored_origin(CocoaRect(rect(30.0, 30.0, 20.0, 20.0)), 420.0, 220.0, tiny);

    assert_eq!(
        origin,
        Some(CocoaPoint {
            x: -160.0,
            y: -70.0
        })
    );
}

#[test]
fn chooses_full_frame_even_outside_the_visible_frame() {
    let selected = CocoaRect(rect(400.0, 880.0, 50.0, 12.0));

    assert_eq!(select_screen(selected, &[primary()]), Some(primary()));
}

#[test]
fn chooses_largest_intersection_when_center_is_in_a_display_gap() {
    let left = screen(
        rect(-1000.0, 0.0, 900.0, 900.0),
        rect(-1000.0, 0.0, 900.0, 800.0),
    );
    let right = screen(
        rect(100.0, 0.0, 900.0, 900.0),
        rect(100.0, 0.0, 900.0, 800.0),
    );
    let selected = CocoaRect(rect(-200.0, 200.0, 500.0, 20.0));

    assert_eq!(select_screen(selected, &[left, right]), Some(right));
    assert_eq!(select_screen(selected, &[right, left]), Some(right));
}

#[test]
fn resolves_a_center_on_a_display_boundary_deterministically() {
    let left = screen(
        rect(-1000.0, 0.0, 1000.0, 900.0),
        rect(-1000.0, 0.0, 1000.0, 800.0),
    );
    let right = primary();
    let selected = CocoaRect(rect(-100.0, 200.0, 200.0, 20.0));

    assert_eq!(select_screen(selected, &[left, right]), Some(right));
    assert_eq!(select_screen(selected, &[right, left]), Some(right));
}

#[test]
fn chooses_the_nearest_display_for_a_selection_outside_all_frames() {
    let left = screen(
        rect(-1000.0, 0.0, 1000.0, 900.0),
        rect(-1000.0, 0.0, 1000.0, 800.0),
    );
    let right = primary();
    let selected = CocoaRect(rect(1800.0, 200.0, 20.0, 20.0));

    assert_eq!(select_screen(selected, &[left, right]), Some(right));
    assert_eq!(select_screen(selected, &[right, left]), Some(right));
}

#[test]
fn handles_displays_left_above_and_below_the_main_screen() {
    let left = screen(
        rect(-1200.0, 0.0, 1200.0, 900.0),
        rect(-1200.0, 0.0, 1200.0, 860.0),
    );
    let above = screen(
        rect(0.0, 900.0, 1000.0, 900.0),
        rect(0.0, 900.0, 1000.0, 860.0),
    );
    let below = screen(
        rect(0.0, -900.0, 1000.0, 900.0),
        rect(0.0, -900.0, 1000.0, 860.0),
    );
    let screens = [primary(), left, above, below];

    assert_eq!(
        select_screen(CocoaRect(rect(-500.0, 300.0, 20.0, 20.0)), &screens),
        Some(left)
    );
    assert_eq!(
        select_screen(CocoaRect(rect(400.0, 1100.0, 20.0, 20.0)), &screens),
        Some(above)
    );
    assert_eq!(
        select_screen(CocoaRect(rect(400.0, -500.0, 20.0, 20.0)), &screens),
        Some(below)
    );
}

#[test]
fn zero_screen_is_the_reference_when_the_key_window_is_on_another_display() {
    let zero = primary();
    let key_window_screen = screen(
        rect(0.0, 900.0, 1200.0, 1200.0),
        rect(0.0, 900.0, 1200.0, 1160.0),
    );
    let screens = [zero, key_window_screen];

    let reference_max_y = zero_screen_max_y(&screens).unwrap();
    let selection = ax_to_cocoa(AxRect(rect(320.0, -250.0, 120.0, 30.0)), reference_max_y).unwrap();

    assert_eq!(reference_max_y, 900.0);
    assert_eq!(selection, CocoaRect(rect(320.0, 1120.0, 120.0, 30.0)));
    assert_eq!(select_screen(selection, &screens), Some(key_window_screen));
}

#[test]
fn toolbar_and_note_heights_use_the_same_stateless_layout() {
    let selection = CocoaRect(rect(200.0, 740.0, 100.0, 20.0));

    let toolbar = anchored_origin(selection, 236.0, 52.0, primary());
    let note = anchored_origin(selection, 420.0, 220.0, primary());
    let toolbar_again = anchored_origin(selection, 236.0, 52.0, primary());

    assert_eq!(toolbar, Some(CocoaPoint { x: 132.0, y: 678.0 }));
    assert_eq!(note, Some(CocoaPoint { x: 40.0, y: 510.0 }));
    assert_eq!(toolbar_again, toolbar);
}

#[test]
fn clamps_deterministically_when_neither_direction_fits() {
    let origin = anchored_origin(
        CocoaRect(rect(300.0, 350.0, 100.0, 20.0)),
        236.0,
        780.0,
        primary(),
    );

    assert_eq!(origin, Some(CocoaPoint { x: 232.0, y: 12.0 }));
}

#[test]
fn rejects_nonfinite_or_negative_geometry() {
    assert_eq!(
        ax_to_cocoa(AxRect(rect(f64::NAN, 0.0, 1.0, 1.0)), 900.0),
        None
    );
    assert_eq!(
        anchored_origin(CocoaRect(rect(0.0, 0.0, 1.0, 1.0)), -1.0, 52.0, primary()),
        None
    );
}
