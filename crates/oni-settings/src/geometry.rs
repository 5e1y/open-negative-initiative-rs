//! `Geometry`'s gestures as Swift's model holds them: mirror, rotate, the remembered framing, the
//! measuring window, the straighten re-fit and the aspect ratio. The crop is in fractions of the
//! upright oriented box, origin bottom left, as Core Image reads it.

use crate::cg::Rect;
use crate::frame::{AspectRatio, Geometry};

/// A width and a height, in pixels.
pub type Size = (f64, f64);

/// Floor for one side of the crop frame: a frame closed on itself could not be reopened.
pub const MIN_CROP: f64 = 0.05;
/// How far inside the crop a proposed measuring window starts, on each side.
pub const MEASURE_WINDOW_INSET: f64 = 0.1;

fn max_x(r: Rect) -> f64 {
    r.x + r.width
}

fn max_y(r: Rect) -> f64 {
    r.y + r.height
}

fn intersection(a: Rect, b: Rect) -> Rect {
    let (x0, y0) = (a.x.max(b.x), a.y.max(b.y));
    let (x1, y1) = (max_x(a).min(max_x(b)), max_y(a).min(max_y(b)));
    if x1 <= x0 || y1 <= y0 {
        return Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };
    }
    Rect { x: x0, y: y0, width: x1 - x0, height: y1 - y0 }
}

/// `Geometry.Framing`: a ratio and its orientation remembered for the next photo; a free frame is none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    pub ratio: AspectRatio,
    pub portrait: bool,
}

impl Framing {
    pub fn new(ratio: Option<AspectRatio>, portrait: bool) -> Option<Framing> {
        Some(Framing { ratio: ratio?, portrait })
    }
}

/// Which side a ratio is held by: the one the hand drags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Driving {
    Width,
    Height,
}

impl Geometry {
    /// The picture and the frame flipped together, so the crop keeps the content it was placed on.
    pub fn mirror(&mut self) {
        self.mirrored = !self.mirrored;
        self.crop.x = 1.0 - max_x(self.crop);
        self.measure_window = self.measure_window.map(|w| Rect { x: 1.0 - max_x(w), ..w });
    }

    pub fn mirror_vertically(&mut self) {
        self.mirrored_vertically = !self.mirrored_vertically;
        self.crop.y = 1.0 - max_y(self.crop);
        self.measure_window = self.measure_window.map(|w| Rect { y: 1.0 - max_y(w), ..w });
    }

    /// The crop removed for the length of a crop gesture, the straightened bounding box shown.
    pub fn without_crop(&self) -> Geometry {
        Geometry { crop: Rect::UNIT, crop_bypassed: true, ..self.clone() }
    }

    /// The image and the frame rotated together, a locked ratio applied again; `image` is raw.
    pub fn rotate(&mut self, quarters: i64, image: Size) {
        let steps = quarters.rem_euclid(4);
        self.quarter_turns = (self.quarter_turns + quarters).rem_euclid(4);
        for _ in 0..steps {
            self.crop = rotated_crop(self.crop);
            self.measure_window = self.measure_window.map(rotated_crop);
        }
        self.apply_ratio(image, None, Driving::Width);
        self.fit_crop_inside_image(image);
    }

    /// The remembered framing taken whatever the frame held, the crop cut to it in place.
    pub fn adopt(&mut self, remembered: Framing, image: Size) {
        self.ratio = Some(remembered.ratio);
        self.portrait = remembered.portrait;
        self.apply_ratio(image, None, Driving::Width);
        self.fit_crop_inside_image(image);
    }

    /// The window over what stage 0 hands on, the crop or the whole frame while it is bypassed;
    /// `None` when it restricts nothing.
    pub fn measure_window_in_crop(&self) -> Option<Rect> {
        let window = self.measure_window?;
        let seen = relative(window, if self.crop_bypassed { Rect::UNIT } else { self.crop });
        (seen != Rect::UNIT).then_some(seen)
    }

    /// Arming an untouched window proposes one inside the crop; a placed one is left alone.
    pub fn propose_measure_window(&mut self, sheet: bool) {
        if self.measure_window.is_none() {
            self.measure_window = Some(proposal(self.crop, sheet));
        }
    }

    pub fn oriented_size(&self, image: Size) -> Size {
        if self.quarter_turns % 2 == 1 { (image.1, image.0) } else { image }
    }

    pub fn cropped_pixel_size(&self, image: Size) -> Size {
        let s = self.oriented_size(image);
        (self.crop.width * s.0, self.crop.height * s.1)
    }

    /// The frame kept inside the rotated photo with no void: under the threshold only the position
    /// clamps, past it both sides scale by one factor.
    pub fn fit_crop_inside_image(&mut self, image: Size) {
        self.normalise_crop();
        let size = self.oriented_size(image);
        if self.angle == 0.0 || size.0 <= 0.0 || size.1 <= 0.0 {
            return;
        }
        let t = f64::from(self.angle).to_radians();
        let (c, s) = (t.cos(), t.sin());
        let crop = self.crop;
        let mut a = crop.width * size.0 / 2.0;
        let mut b = crop.height * size.1 / 2.0;
        let mut dx = (crop.x + crop.width / 2.0 - 0.5) * size.0;
        let mut dy = (crop.y + crop.height / 2.0 - 0.5) * size.1;
        let mut footprint = (a * c.abs() + b * s.abs(), a * s.abs() + b * c.abs());
        if footprint.0 <= 0.0 || footprint.1 <= 0.0 {
            return;
        }
        if footprint.0 > size.0 / 2.0 || footprint.1 > size.1 / 2.0 {
            let k = (size.0 / 2.0 / footprint.0).min(size.1 / 2.0 / footprint.1);
            a *= k;
            b *= k;
            footprint = (footprint.0 * k, footprint.1 * k);
        }
        // The centre is clamped in the image's own frame, rotated by −θ, then put back upright.
        let room = ((size.0 / 2.0 - footprint.0).max(0.0), (size.1 / 2.0 - footprint.1).max(0.0));
        let u = (dx * c + dy * s).clamp(-room.0, room.0);
        let v = (-dx * s + dy * c).clamp(-room.1, room.1);
        dx = u * c - v * s;
        dy = u * s + v * c;
        self.crop = Rect {
            x: (dx + size.0 / 2.0 - a) / size.0,
            y: (dy + size.1 / 2.0 - b) / size.1,
            width: 2.0 * a / size.0,
            height: 2.0 * b / size.1,
        };
    }

    /// One straightening gesture expressed on its starting frame, so the shrink never ratchets.
    pub fn straightened(&self, angle: f32, image: Size) -> Geometry {
        let mut fitted = self.clone();
        fitted.angle = angle;
        fitted.fit_crop_inside_image(image);
        fitted
    }

    /// The frame fitted to the ratio, `anchor` the point that stays put: the corner opposite the
    /// one dragged, or the centre; `image` is raw, oriented here.
    pub fn apply_ratio(&mut self, image: Size, anchor: Option<(f64, f64)>, driving: Driving) {
        let Some(ratio) = self.ratio else { return };
        let size = self.oriented_size(image);
        if size.0 <= 0.0 || size.1 <= 0.0 {
            return;
        }
        let wanted = if self.portrait { 1.0 / ratio.value() } else { ratio.value() };
        let target = wanted * size.1 / size.0;
        let crop = self.crop;
        let point = anchor.unwrap_or((crop.x + crop.width / 2.0, crop.y + crop.height / 2.0));
        let relative = (
            if crop.width > 0.0 { (point.0 - crop.x) / crop.width } else { 0.5 },
            if crop.height > 0.0 { (point.1 - crop.y) / crop.height } else { 0.5 },
        );
        let (mut width, mut height);
        match driving {
            Driving::Width => {
                width = crop.width;
                height = width / target;
                if height > 1.0 {
                    height = crop.height.min(1.0);
                    width = height * target;
                }
            }
            Driving::Height => {
                height = crop.height;
                width = height * target;
                if width > 1.0 {
                    width = crop.width.min(1.0);
                    height = width / target;
                }
            }
        }
        // The floor carries to the other side, never alone: crushing one side would break the ratio.
        if width < MIN_CROP {
            width = MIN_CROP;
            height = width / target;
        }
        if height < MIN_CROP {
            height = MIN_CROP;
            width = height * target;
        }
        self.crop = Rect { x: point.0 - relative.0 * width, y: point.1 - relative.1 * height, width: width.min(1.0), height: height.min(1.0) };
        self.normalise_crop();
    }

    pub fn normalise_crop(&mut self) {
        let mut c = self.crop;
        c.width = c.width.clamp(MIN_CROP, 1.0);
        c.height = c.height.clamp(MIN_CROP, 1.0);
        c.x = c.x.clamp(0.0, 1.0 - c.width);
        c.y = c.y.clamp(0.0, 1.0 - c.height);
        self.crop = c;
    }
}

/// `rect` seen from inside `base`; a rect missing `base` reads as the whole of it.
pub fn relative(rect: Rect, base: Rect) -> Rect {
    let cut = intersection(rect, base);
    if base.width <= 0.0 || base.height <= 0.0 || cut.width <= 0.0 || cut.height <= 0.0 {
        return Rect::UNIT;
    }
    Rect { x: (cut.x - base.x) / base.width, y: (cut.y - base.y) / base.height, width: cut.width / base.width, height: cut.height / base.height }
}

/// The inverse of `relative`.
pub fn absolute(rect: Rect, base: Rect) -> Rect {
    Rect { x: base.x + rect.x * base.width, y: base.y + rect.y * base.height, width: rect.width * base.width, height: rect.height * base.height }
}

pub fn proposal(crop: Rect, sheet: bool) -> Rect {
    let inset = if sheet { 0.0 } else { MEASURE_WINDOW_INSET };
    let (dx, dy) = (crop.width * inset, crop.height * inset);
    Rect { x: crop.x + dx, y: crop.y + dy, width: crop.width - 2.0 * dx, height: crop.height - 2.0 * dy }
}

pub fn rotated_crop(crop: Rect) -> Rect {
    Rect { x: 1.0 - max_y(crop), y: crop.x, width: crop.height, height: crop.width }
}

/// `Geometry.pixelLabel`.
pub fn pixel_label(size: Size) -> String {
    format!("{:.0} × {:.0} px", size.0, size.1)
}

/// The largest same-proportion rectangle inscribed in the straightened image, as a factor.
pub fn straighten_scale(angle: f32, size: Size) -> f64 {
    if angle == 0.0 || size.0 <= 0.0 || size.1 <= 0.0 {
        return 1.0;
    }
    let t = f64::from(angle).abs().to_radians();
    let (c, s) = (t.cos(), t.sin());
    (size.0 / (size.0 * c + size.1 * s)).min(size.1 / (size.0 * s + size.1 * c))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: Size = (6000.0, 4000.0);

    fn close(a: Rect, b: Rect) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9 && (a.width - b.width).abs() < 1e-9 && (a.height - b.height).abs() < 1e-9
    }

    #[test]
    fn a_mirror_keeps_the_crop_on_its_content() {
        let mut g = Geometry { crop: Rect { x: 0.1, y: 0.2, width: 0.3, height: 0.4 }, measure_window: Some(Rect { x: 0.0, y: 0.0, width: 0.5, height: 0.5 }), ..Geometry::default() };
        g.mirror();
        assert!(g.mirrored && close(g.crop, Rect { x: 0.6, y: 0.2, width: 0.3, height: 0.4 }));
        assert!(close(g.measure_window.unwrap(), Rect { x: 0.5, y: 0.0, width: 0.5, height: 0.5 }));
        g.mirror_vertically();
        assert!(close(g.crop, Rect { x: 0.6, y: 0.4, width: 0.3, height: 0.4 }));
        // Twin: mirroring twice is the frame it started from.
        g.mirror();
        g.mirror_vertically();
        assert!(!g.mirrored && close(g.crop, Rect { x: 0.1, y: 0.2, width: 0.3, height: 0.4 }));
    }

    #[test]
    fn four_quarter_turns_return_the_frame_and_one_swaps_its_sides() {
        let start = Rect { x: 0.1, y: 0.2, width: 0.3, height: 0.4 };
        let mut g = Geometry { crop: start, ..Geometry::default() };
        g.rotate(1, RAW);
        assert_eq!(g.quarter_turns, 1);
        assert!(close(g.crop, Rect { x: 0.4, y: 0.1, width: 0.4, height: 0.3 }));
        assert_eq!(g.oriented_size(RAW), (4000.0, 6000.0));
        g.rotate(3, RAW);
        assert!(g.quarter_turns == 0 && close(g.crop, start));
        // Twin: a negative turn goes the other way round.
        let mut back = Geometry { crop: start, ..Geometry::default() };
        back.rotate(-1, RAW);
        assert_eq!(back.quarter_turns, 3);
        assert!(!close(back.crop, Rect { x: 0.4, y: 0.1, width: 0.4, height: 0.3 }));
    }

    #[test]
    fn a_ratio_cuts_the_frame_about_its_anchor_and_holds_the_floor() {
        let mut g = Geometry { ratio: Some(AspectRatio::Square), ..Geometry::default() };
        g.apply_ratio(RAW, None, Driving::Width);
        // Square on a 3:2 frame: the width shrinks to two thirds about the centre.
        assert!(close(g.crop, Rect { x: 1.0 / 6.0, y: 0.0, width: 2.0 / 3.0, height: 1.0 }), "{:?}", g.crop);
        let px = g.cropped_pixel_size(RAW);
        assert!((px.0 - px.1).abs() < 1e-6, "square in pixels: {px:?}");
        // Portrait flips the ratio.
        let mut p = Geometry { ratio: Some(AspectRatio::ThreeTwo), portrait: true, ..Geometry::default() };
        p.apply_ratio(RAW, None, Driving::Width);
        let px = p.cropped_pixel_size(RAW);
        assert!((px.1 / px.0 - 1.5).abs() < 1e-6, "{px:?}");
        // Anchored at the bottom-left corner, that corner stays.
        let mut a = Geometry { crop: Rect { x: 0.2, y: 0.1, width: 0.6, height: 0.6 }, ratio: Some(AspectRatio::Square), ..Geometry::default() };
        a.apply_ratio(RAW, Some((0.2, 0.1)), Driving::Width);
        assert!((a.crop.x - 0.2).abs() < 1e-9 && (a.crop.y - 0.1).abs() < 1e-9);
        // Twin: no ratio leaves the frame as it was.
        let mut free = Geometry { crop: Rect { x: 0.2, y: 0.1, width: 0.6, height: 0.6 }, ..Geometry::default() };
        free.apply_ratio(RAW, None, Driving::Width);
        assert!(close(free.crop, Rect { x: 0.2, y: 0.1, width: 0.6, height: 0.6 }));
    }

    #[test]
    fn straightening_refits_the_frame_inside_the_turned_photo() {
        let g = Geometry::default();
        let s = g.straightened(5.0, RAW);
        // The full frame shrinks by the inscribed factor, about the centre.
        let k = straighten_scale(5.0, RAW);
        assert!((s.crop.width - k).abs() < 1e-6 && (s.crop.height - k).abs() < 1e-6, "{:?} for {k}", s.crop);
        assert!((s.crop.x + s.crop.width / 2.0 - 0.5).abs() < 1e-9);
        // Every corner of the kept frame lies inside the turned photo.
        let t = 5f64.to_radians();
        for (fx, fy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let px = ((s.crop.x + fx * s.crop.width - 0.5) * RAW.0, (s.crop.y + fy * s.crop.height - 0.5) * RAW.1);
            let (u, v) = (px.0 * t.cos() + px.1 * t.sin(), -px.0 * t.sin() + px.1 * t.cos());
            assert!(u.abs() <= RAW.0 / 2.0 + 1e-6 && v.abs() <= RAW.1 / 2.0 + 1e-6, "corner ({fx}, {fy}) outside: {u} {v}");
        }
        // Twin: from the gesture's starting frame, going back to zero restores it; refitting live would ratchet.
        assert!(close(s.straightened(0.0, RAW).crop, s.crop));
        assert!(close(g.straightened(0.0, RAW).crop, Rect::UNIT));
    }

    #[test]
    fn the_measuring_window_reads_inside_the_crop() {
        let mut g = Geometry { crop: Rect { x: 0.2, y: 0.2, width: 0.6, height: 0.6 }, ..Geometry::default() };
        assert_eq!(g.measure_window_in_crop(), None, "an untouched window restricts nothing");
        g.propose_measure_window(false);
        let w = g.measure_window.unwrap();
        assert!(close(w, Rect { x: 0.26, y: 0.26, width: 0.48, height: 0.48 }), "{w:?}");
        let inside = g.measure_window_in_crop().unwrap();
        assert!(close(inside, Rect { x: 0.1, y: 0.1, width: 0.8, height: 0.8 }), "{inside:?}");
        assert!(close(absolute(inside, g.crop), w));
        // Twin: a placed window is never replaced by a new proposal.
        g.measure_window = Some(Rect { x: 0.3, y: 0.3, width: 0.1, height: 0.1 });
        g.propose_measure_window(true);
        assert!(close(g.measure_window.unwrap(), Rect { x: 0.3, y: 0.3, width: 0.1, height: 0.1 }));
        assert_eq!(pixel_label((4000.4, 2666.6)), "4000 × 2667 px");
    }
}
