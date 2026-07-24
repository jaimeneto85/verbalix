use crate::domain::Rect;

pub const OVERLAY_GAP: f64 = 10.0;
pub const SCREEN_MARGIN: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxRect(pub Rect);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CocoaRect(pub Rect);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenFrame {
    pub full: CocoaRect,
    pub visible: CocoaRect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CocoaPoint {
    pub x: f64,
    pub y: f64,
}

pub fn ax_to_cocoa(rect: AxRect, main_max_y: f64) -> Option<CocoaRect> {
    valid_rect(rect.0)?;
    if !main_max_y.is_finite() {
        return None;
    }
    Some(CocoaRect(Rect {
        x: rect.0.x,
        y: main_max_y - rect.0.y - rect.0.height,
        width: rect.0.width,
        height: rect.0.height,
    }))
}

#[cfg(test)]
pub fn cocoa_to_ax(rect: CocoaRect, main_max_y: f64) -> Option<AxRect> {
    valid_rect(rect.0)?;
    if !main_max_y.is_finite() {
        return None;
    }
    Some(AxRect(Rect {
        x: rect.0.x,
        y: main_max_y - rect.0.y - rect.0.height,
        width: rect.0.width,
        height: rect.0.height,
    }))
}

pub fn select_screen(selection: CocoaRect, screens: &[ScreenFrame]) -> Option<ScreenFrame> {
    valid_rect(selection.0)?;
    let center = center(selection.0);
    let valid = screens
        .iter()
        .copied()
        .filter(|screen| valid_screen(*screen));
    let containing: Vec<_> = valid
        .clone()
        .filter(|screen| contains(screen.full.0, center))
        .collect();
    best_screen(
        selection.0,
        center,
        if containing.is_empty() {
            valid.collect()
        } else {
            containing
        },
    )
}

pub fn anchored_origin(
    selection: CocoaRect,
    width: f64,
    height: f64,
    screen: ScreenFrame,
) -> Option<CocoaPoint> {
    valid_rect(selection.0)?;
    if !valid_screen(screen) || !valid_extent(width) || !valid_extent(height) {
        return None;
    }
    let visible = screen.visible.0;
    let x = selection.0.x + selection.0.width / 2.0 - width / 2.0;
    let above = selection.0.y + selection.0.height + OVERLAY_GAP;
    let below = selection.0.y - OVERLAY_GAP - height;
    let min_y = visible.y + SCREEN_MARGIN;
    let max_y = visible.y + visible.height - height - SCREEN_MARGIN;
    let y = if fits(above, height, visible) {
        above
    } else if fits(below, height, visible) {
        below
    } else {
        clamp_axis(above, min_y, max_y, visible.y, visible.height, height)
    };
    Some(CocoaPoint {
        x: clamp_axis(
            x,
            visible.x + SCREEN_MARGIN,
            visible.x + visible.width - width - SCREEN_MARGIN,
            visible.x,
            visible.width,
            width,
        ),
        y,
    })
}

fn valid_rect(rect: Rect) -> Option<()> {
    let values = [rect.x, rect.y, rect.width, rect.height];
    (values.into_iter().all(f64::is_finite) && rect.width >= 0.0 && rect.height >= 0.0)
        .then_some(())
}

fn valid_screen(screen: ScreenFrame) -> bool {
    valid_rect(screen.full.0).is_some()
        && valid_rect(screen.visible.0).is_some()
        && screen.full.0.width > 0.0
        && screen.full.0.height > 0.0
        && screen.visible.0.width > 0.0
        && screen.visible.0.height > 0.0
}

fn valid_extent(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn center(rect: Rect) -> CocoaPoint {
    CocoaPoint {
        x: rect.x + rect.width / 2.0,
        y: rect.y + rect.height / 2.0,
    }
}

fn contains(rect: Rect, point: CocoaPoint) -> bool {
    point.x >= rect.x
        && point.x < rect.x + rect.width
        && point.y >= rect.y
        && point.y < rect.y + rect.height
}

fn best_screen(
    selection: Rect,
    center: CocoaPoint,
    screens: Vec<ScreenFrame>,
) -> Option<ScreenFrame> {
    screens.into_iter().reduce(|best, candidate| {
        if is_better(selection, center, candidate, best) {
            candidate
        } else {
            best
        }
    })
}

fn is_better(
    selection: Rect,
    center: CocoaPoint,
    candidate: ScreenFrame,
    current: ScreenFrame,
) -> bool {
    let candidate_area = intersection_area(selection, candidate.full.0);
    let current_area = intersection_area(selection, current.full.0);
    match candidate_area.total_cmp(&current_area) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => {
            let candidate_distance = distance_squared(center, candidate.full.0);
            let current_distance = distance_squared(center, current.full.0);
            match candidate_distance.total_cmp(&current_distance) {
                std::cmp::Ordering::Less => true,
                std::cmp::Ordering::Greater => false,
                std::cmp::Ordering::Equal => {
                    deterministic_key(candidate) < deterministic_key(current)
                }
            }
        }
    }
}

fn intersection_area(first: Rect, second: Rect) -> f64 {
    let width = (first.x + first.width).min(second.x + second.width) - first.x.max(second.x);
    let height = (first.y + first.height).min(second.y + second.height) - first.y.max(second.y);
    width.max(0.0) * height.max(0.0)
}

fn distance_squared(point: CocoaPoint, rect: Rect) -> f64 {
    let x = point.x.clamp(rect.x, rect.x + rect.width);
    let y = point.y.clamp(rect.y, rect.y + rect.height);
    (point.x - x).powi(2) + (point.y - y).powi(2)
}

fn deterministic_key(screen: ScreenFrame) -> (u64, u64, u64, u64) {
    (
        screen.full.0.x.to_bits(),
        screen.full.0.y.to_bits(),
        screen.full.0.width.to_bits(),
        screen.full.0.height.to_bits(),
    )
}

fn fits(origin: f64, extent: f64, visible: Rect) -> bool {
    origin >= visible.y + SCREEN_MARGIN
        && origin + extent <= visible.y + visible.height - SCREEN_MARGIN
}

fn clamp_axis(
    value: f64,
    minimum: f64,
    maximum: f64,
    frame_origin: f64,
    frame_extent: f64,
    overlay_extent: f64,
) -> f64 {
    if minimum <= maximum {
        value.clamp(minimum, maximum)
    } else {
        frame_origin + (frame_extent - overlay_extent) / 2.0
    }
}

#[cfg(test)]
#[path = "overlay_geometry_tests.rs"]
mod tests;
