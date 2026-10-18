//! `FrameOverlay`'s arithmetic: a handle dragged moves the frame in the image's fractions, the crop
//! held to its ratio about the point opposite the hand and fitted inside the turned photo, the
//! measuring window bounded; and where a frame lands on the canvas.

use oni_settings::cg::Rect;
use oni_settings::frame::Geometry;
use oni_settings::geometry::{self, Driving, MIN_CROP, Size};

/// 0 the body, 1 to 4 the corners top-left, top-right, bottom-left, bottom-right, 5 to 8 the edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Body,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Left,
    Right,
    Top,
    Bottom,
}

impl Handle {
    pub fn from_index(i: i32) -> Option<Handle> {
        use Handle::*;
        [Body, TopLeft, TopRight, BottomLeft, BottomRight, Left, Right, Top, Bottom].get(usize::try_from(i).ok()?).copied()
    }

    fn moves_left(self) -> bool {
        matches!(self, Handle::Left | Handle::TopLeft | Handle::BottomLeft)
    }

    fn moves_right(self) -> bool {
        matches!(self, Handle::Right | Handle::TopRight | Handle::BottomRight)
    }

    fn moves_top(self) -> bool {
        matches!(self, Handle::Top | Handle::TopLeft | Handle::TopRight)
    }

    fn moves_bottom(self) -> bool {
        matches!(self, Handle::Bottom | Handle::BottomLeft | Handle::BottomRight)
    }

    /// The side that drives the ratio: the top and bottom edges change only the height.
    pub fn driving(self) -> Driving {
        if matches!(self, Handle::Top | Handle::Bottom) { Driving::Height } else { Driving::Width }
    }

    /// The point that stays fixed, opposite the one held, in the frame's fractions, origin bottom.
    pub fn opposite(self, r: Rect) -> (f64, f64) {
        let x = if self.moves_left() { r.x + r.width } else if self.moves_right() { r.x } else { r.x + r.width / 2.0 };
        let y = if self.moves_bottom() { r.y + r.height } else if self.moves_top() { r.y } else { r.y + r.height / 2.0 };
        (x, y)
    }
}

/// `FrameOverlay.apply`: the frame at the gesture's start moved by a translation in points over a
/// displayed frame of `frame` points, y down on screen and up in the fractions.
pub fn apply(handle: Handle, from: Rect, frame: Size, translation: Size) -> Rect {
    if frame.0 <= 0.0 || frame.1 <= 0.0 {
        return from;
    }
    let (dx, dy) = (translation.0 / frame.0, -translation.1 / frame.1);
    let mut c = from;
    if handle == Handle::Body {
        c.x = from.x + dx;
        c.y = from.y + dy;
        return c;
    }
    let (max_x, max_y) = (from.x + from.width, from.y + from.height);
    if handle.moves_left() {
        let x = (from.x + dx).min(max_x - MIN_CROP).max(0.0);
        c.x = x;
        c.width = max_x - x;
    } else if handle.moves_right() {
        c.width = (from.width + dx).max(MIN_CROP).min(1.0 - from.x);
    }
    if handle.moves_bottom() {
        let y = (from.y + dy).min(max_y - MIN_CROP).max(0.0);
        c.y = y;
        c.height = max_y - y;
    } else if handle.moves_top() {
        c.height = (from.height + dy).max(MIN_CROP).min(1.0 - from.y);
    }
    c
}

/// `CropOverlay.constrain`: the ratio about the point opposite the hand, then the fit under rotation.
pub fn constrained_crop(geometry: &Geometry, handle: Handle, proposed: Rect, image: Size) -> Geometry {
    let mut g = geometry.clone();
    g.crop = proposed;
    g.apply_ratio(image, Some(handle.opposite(proposed)), handle.driving());
    g.fit_crop_inside_image(image);
    g
}

/// `MeasureWindowOverlay.bounded`.
pub fn bounded(proposed: Rect) -> Rect {
    let mut r = proposed;
    r.width = r.width.clamp(MIN_CROP, 1.0);
    r.height = r.height.clamp(MIN_CROP, 1.0);
    r.x = r.x.clamp(0.0, 1.0 - r.width);
    r.y = r.y.clamp(0.0, 1.0 - r.height);
    r
}

/// The window as the overlay draws it: on the upright box while framing, inside the crop otherwise.
pub fn window_shown(g: &Geometry, framing: bool) -> Rect {
    let window = g.measure_window.unwrap_or(g.crop);
    if framing { window } else { geometry::relative(window, g.crop) }
}

/// The window put back into the frame's fractions from where the overlay drew it.
pub fn window_stored(g: &Geometry, drawn: Rect, framing: bool) -> Rect {
    if framing { drawn } else { geometry::absolute(drawn, g.crop) }
}

/// A frame's rectangle in points on the canvas, from its fractions of an image frame drawn at
/// `image` = (x, y, width, height) points; the y inversion reads Core Image's bottom origin.
pub fn shown(rect: Rect, image: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    (image.0 + rect.x * image.2, image.1 + (1.0 - rect.y - rect.height) * image.3, rect.width * image.2, rect.height * image.3)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: Size = (6000.0, 4000.0);

    #[test]
    fn a_corner_moves_its_two_sides_and_the_body_moves_all() {
        let from = Rect { x: 0.2, y: 0.2, width: 0.6, height: 0.6 };
        // The top-left corner dragged 60 points right and 40 down over a 600 × 400 point frame.
        let moved = apply(Handle::TopLeft, from, (600.0, 400.0), (60.0, 40.0));
        assert!((moved.x - 0.3).abs() < 1e-9 && (moved.width - 0.5).abs() < 1e-9, "{moved:?}");
        assert!((moved.y - 0.2).abs() < 1e-9 && (moved.height - 0.5).abs() < 1e-9, "the top comes down: {moved:?}");
        let body = apply(Handle::Body, from, (600.0, 400.0), (60.0, 40.0));
        assert!((body.x - 0.3).abs() < 1e-9 && (body.y - 0.1).abs() < 1e-9 && body.width == 0.6);
        // Twin: the floor holds a side the hand pushes past the other.
        let closed = apply(Handle::Right, from, (600.0, 400.0), (-1000.0, 0.0));
        assert!((closed.width - MIN_CROP).abs() < 1e-9);
    }

    #[test]
    fn a_locked_ratio_keeps_the_corner_opposite_the_hand() {
        let g = Geometry { ratio: Some(oni_settings::frame::AspectRatio::Square), crop: Rect { x: 0.2, y: 0.2, width: 0.4, height: 0.6 }, ..Geometry::default() };
        let proposed = apply(Handle::BottomRight, g.crop, (600.0, 400.0), (60.0, 0.0));
        let fitted = constrained_crop(&g, Handle::BottomRight, proposed, RAW);
        // The top-left corner, opposite the bottom-right held, stays where it was.
        assert!((fitted.crop.x - 0.2).abs() < 1e-9, "{:?}", fitted.crop);
        assert!((fitted.crop.y + fitted.crop.height - 0.8).abs() < 1e-9, "{:?}", fitted.crop);
        let px = fitted.cropped_pixel_size(RAW);
        assert!((px.0 - px.1).abs() < 1e-6, "square: {px:?}");
        // Twin: anchored at the centre instead, the top edge would move with the height.
        let mut centred = g.clone();
        centred.crop = proposed;
        centred.apply_ratio(RAW, None, Driving::Width);
        assert!((centred.crop.y + centred.crop.height - 0.8).abs() > 1e-3);
    }

    #[test]
    fn the_window_is_drawn_inside_the_crop_outside_framing() {
        let g = Geometry { crop: Rect { x: 0.2, y: 0.2, width: 0.5, height: 0.5 }, measure_window: Some(Rect { x: 0.3, y: 0.3, width: 0.25, height: 0.25 }), ..Geometry::default() };
        let inside = window_shown(&g, false);
        assert!((inside.x - 0.2).abs() < 1e-9 && (inside.width - 0.5).abs() < 1e-9, "{inside:?}");
        let back = window_stored(&g, inside, false);
        assert!((back.x - 0.3).abs() < 1e-9 && (back.width - 0.25).abs() < 1e-9);
        // Twin: framing, the window is drawn on the upright box as it is stored.
        assert_eq!(window_shown(&g, true), g.measure_window.unwrap());
        // On a 600 × 400 point image at (10, 20), the top of a frame at y 0.75 is a quarter down.
        let s = shown(Rect { x: 0.5, y: 0.5, width: 0.5, height: 0.25 }, (10.0, 20.0, 600.0, 400.0));
        assert_eq!(s, (310.0, 120.0, 300.0, 100.0));
    }
}
