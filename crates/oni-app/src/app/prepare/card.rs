//! The card: each frame in its scan's shape, dissolving into the next, and the base picked on its scan.

use super::*;
use std::time::Duration;

/// The least time from one dissolve's start to the next's, however short `Motion.dissolve`: two dissolves
/// make a flash, so the card flashes under three times a second (WCAG 2.3.1).
pub(super) const PACE: Duration = Duration::from_millis(200);

/// The card's two layers: the frame each holds, whether the second lies on top, and when the last
/// dissolve began; a frame asked within the pace waits for its end, the latest asked laid.
#[derive(Default)]
pub(super) struct Layers {
    paths: [Option<String>; 2],
    second: bool,
    began: Option<Instant>,
    waiting: slint::Timer,
}

impl Layers {
    /// The frame on top, by path.
    pub(super) fn top(&self) -> Option<&str> {
        self.paths[usize::from(self.second)].as_deref()
    }
}

/// What a frame asked does to the layers.
#[derive(Debug, PartialEq)]
enum Lay {
    /// Its picture set on the top layer, which shows that frame or none yet.
    InPlace,
    /// Dissolved in once the pace allows, this long from now.
    After(Duration),
    Dissolve,
}

fn lay(l: &Layers, path: Option<&str>, now: Instant) -> Lay {
    let top = l.top();
    if top.is_none() || top == path {
        return Lay::InPlace;
    }
    match l.began.map(|b| PACE.saturating_sub(now.duration_since(b))) {
        Some(left) if !left.is_zero() => Lay::After(left),
        _ => Lay::Dissolve,
    }
}

/// The frame the card shows, by path, and its picture: the preview, else the tile, else the thumbnail.
pub(super) fn card_picture(s: &State) -> (Option<String>, Image) {
    let Some(f) = s.prepare.shown_frame() else { return (None, Image::default()) };
    let picture = match &s.prepare.card {
        Some((path, image)) if *path == f.path => image.clone(),
        _ => s.prepare.tiles.get(&f.path).cloned().unwrap_or_default(),
    };
    let picture = if picture.size().width == 0 { s.thumbs.get(&f.fingerprint).unwrap_or_default() } else { picture };
    (Some(f.path.clone()), picture)
}

impl Ctl {
    // MARK: - The card and the base's scan

    /// The frame on the card's top layer fitted in the card, so the window lies over the frame shown.
    pub(in crate::app) fn card_box(&self) -> Option<(f64, f64, f64, f64)> {
        let w = self.window()?;
        let p = w.global::<Preparing>();
        let r = &self.state.borrow().prepare;
        let top = r.layers.top().and_then(|t| r.frames.iter().find(|f| f.path == t));
        let aspect = top.or(r.shown_frame()).and_then(|f| f.aspect).unwrap_or(r.aspect);
        Some(fitted(aspect, (f64::from(p.get_card_width()), f64::from(p.get_card_height()))))
    }

    /// Each layer's frame where its crop lies in its own turned scan, in the card.
    pub(super) fn layer_boxes(&self, size: (f64, f64)) -> Vec<f32> {
        let r = &self.state.borrow().prepare;
        let laid = |path: &Option<String>| {
            let f = r.frames.iter().find(|f| Some(&f.path) == path.as_ref());
            let (x, y, w, h) = frames::shown(f.map_or(Rect::UNIT, |f| f.crop), fitted(f.and_then(|f| f.aspect).unwrap_or(r.aspect), size));
            [x as f32, y as f32, w as f32, h as f32]
        };
        r.layers.paths.iter().flat_map(laid).collect()
    }

    /// The shown frame's picture laid on the card: in place, or dissolving in over the frame shown.
    pub(super) fn lay_card(&self, path: Option<String>, picture: Image) {
        let Some(w) = self.window() else { return };
        let p = w.global::<Preparing>();
        let (layer, cleared, second) = {
            let mut s = self.state.borrow_mut();
            let l = &mut s.prepare.layers;
            let now = Instant::now();
            let (layer, cleared) = match lay(l, path.as_deref(), now) {
                Lay::After(left) => {
                    l.waiting.start(slint::TimerMode::SingleShot, left, || with_shell(|c| c.relay_card()));
                    return;
                }
                Lay::InPlace => (usize::from(l.second), l.top().is_none()),
                Lay::Dissolve => {
                    l.second = !l.second;
                    l.began = Some(now);
                    (usize::from(l.second), false)
                }
            };
            l.paths[layer] = path;
            if cleared {
                l.paths[1 - layer] = None;
            }
            (layer, cleared, l.second)
        };
        let set = |layer: usize, picture: Image| if layer == 0 { p.set_card_a(picture) } else { p.set_card_b(picture) };
        set(layer, picture);
        if cleared {
            set(1 - layer, Image::default());
        }
        p.set_card_second(second);
    }

    /// The latest frame asked, laid once the pace it waited on has passed.
    fn relay_card(&self) {
        let (path, picture) = card_picture(&self.state.borrow());
        self.lay_card(path, picture);
        self.publish_card();
    }

    pub(in crate::app) fn window_grabbed(self: &Rc<Self>, handle: i32) {
        let (Some(handle), Some(b)) = (Handle::from_index(handle), self.card_box()) else { return };
        let r = &mut self.state.borrow_mut().prepare;
        r.window_gesture = Some((handle, r.window, (b.2, b.3)));
    }

    /// `MeasureWindowOverlay.bounded`: the window kept inside the whole scan, never under 5 %.
    pub(in crate::app) fn window_dragged(self: &Rc<Self>, dx: f64, dy: f64) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            let Some((handle, from, size)) = r.window_gesture else { return };
            r.window = frames::bounded(frames::apply(handle, from, size, (dx, dy)));
        }
        self.publish_card();
    }

    /// A key on the floor holding the keyboard: the base's picker or the measuring window, each step's
    /// one thing to place; any other step, or the measure under way, leaves it to the photos.
    pub(in crate::app) fn floor_key(self: &Rc<Self>, text: &str, shift: bool, option: bool) -> bool {
        let (step, measuring) = {
            let r = &self.state.borrow().prepare;
            (r.step, r.progress.is_some())
        };
        match step {
            Step::Base if !measuring => self.base_key(text, shift, option),
            Step::Window if !measuring => self.window_key(text, shift, option),
            _ => false,
        }
    }

    /// The arrows move the picker over the scan, its loupe with it, from where the hand or the pick
    /// left it; Return picks the film under it, as a click there does.
    fn base_key(self: &Rc<Self>, text: &str, shift: bool, option: bool) -> bool {
        let Some((rect, _)) = self.scan_rect().filter(|(r, _)| r.2 > 0.0 && r.3 > 0.0) else { return false };
        let inside = |(x, y): (f64, f64)| (x.clamp(rect.0, rect.0 + rect.2), y.clamp(rect.1, rect.1 + rect.3));
        let at = {
            let r = &self.state.borrow().prepare;
            let shown = r.base_shown().map(|f| f.path.clone());
            let picked = r.base.as_ref().filter(|b| Some(&b.path) == shown.as_ref()).map(|b| (rect.0 + b.unit.0 * rect.2, rect.1 + b.unit.1 * rect.3));
            r.pointer.or(picked).map(inside)
        };
        if matches!(text, "\r" | "\n") {
            let Some((x, y)) = at else { return false };
            self.base_clicked(x, y);
            return true;
        }
        let Some((Handle::Body, (dx, dy))) = frames::arrow(text, shift, option) else { return false };
        let (x, y) = at.unwrap_or((rect.0 + rect.2 / 2.0, rect.1 + rect.3 / 2.0));
        self.state.borrow_mut().prepare.pointer = Some(inside((x + dx, y + dy)));
        self.publish_base();
        true
    }

    /// The arrows move the measuring window over the card, ⌥ its right and bottom edges, as a drag does.
    fn window_key(self: &Rc<Self>, text: &str, shift: bool, option: bool) -> bool {
        let Some((handle, by)) = frames::arrow(text, shift, option) else { return false };
        let Some(b) = self.card_box() else { return true };
        {
            let r = &mut self.state.borrow_mut().prepare;
            r.window = frames::bounded(frames::apply(handle, r.window, (b.2, b.3), by));
        }
        self.publish_card();
        true
    }

    /// The scan's rectangle in the base area, and the scan's own size in pixels.
    pub(in crate::app) fn scan_rect(&self) -> Option<((f64, f64, f64, f64), (usize, usize))> {
        let w = self.window()?;
        let p = w.global::<Preparing>();
        let r = &self.state.borrow().prepare;
        let path = r.base_shown()?.path.clone();
        let (_, _, size, _) = r.scan.as_ref().filter(|(p, _, _, _)| *p == path)?;
        let rect = fitted_image((size.0 as f64, size.1 as f64), (f64::from(p.get_base_width()), f64::from(p.get_base_height())));
        Some((rect, *size))
    }

    /// A click inside the scan picks the bare film there, with a key's click, and the walk goes on by
    /// itself; the step, back to it, shows the pick and takes another. One landing nowhere stays.
    pub(in crate::app) fn base_clicked(self: &Rc<Self>, x: f64, y: f64) {
        let Some((rect, _)) = self.scan_rect() else { return };
        // A scan not laid yet has no size: a click there would pick a point of none.
        if !(rect.2 > 0.0 && rect.3 > 0.0) || x < rect.0 || y < rect.1 || x > rect.0 + rect.2 || y > rect.1 + rect.3 {
            return;
        }
        {
            let mut s = self.state.borrow_mut();
            let radius = base_radius(&s);
            let r = &mut s.prepare;
            let Some(path) = r.base_shown().map(|f| f.path.clone()) else { return };
            r.base = Some(BasePick { path, unit: ((x - rect.0) / rect.2, (y - rect.1) / rect.3), radius });
        }
        if let Some(w) = self.window() {
            w.global::<crate::Clicks>().invoke_press();
        }
        self.roll_advance();
    }
}

/// The colour the pick sampled, as the scan shows it: its pixels' mean inside the disc.
pub(super) fn sampled(scan: &Image, unit: (f64, f64), radius: f64) -> slint::Color {
    let Some(pixels) = scan.to_rgba8() else { return slint::Color::default() };
    let [r, g, b] = disc_mean(pixels.as_bytes(), pixels.width() as usize, pixels.height() as usize, unit, radius);
    slint::Color::from_rgb_u8(r, g, b)
}

/// The mean of an RGBA picture's pixels whose centres lie in the disc at `unit`, `radius` pixels
/// wide; the pixel under its centre when the disc holds none.
fn disc_mean(rgba: &[u8], width: usize, height: usize, unit: (f64, f64), radius: f64) -> [u8; 3] {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return [0; 3];
    }
    let (cx, cy) = (unit.0 * width as f64, unit.1 * height as f64);
    let span = |c: f64, end: usize| ((c - radius).floor().max(0.0) as usize, ((c + radius).ceil().max(0.0) as usize).min(end));
    let ((x0, x1), (y0, y1)) = (span(cx, width), span(cy, height));
    let (mut sum, mut count) = ([0u64; 3], 0u64);
    for y in y0..y1 {
        for x in x0..x1 {
            if (x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2) > radius * radius {
                continue;
            }
            let at = (y * width + x) * 4;
            for (s, v) in sum.iter_mut().zip(&rgba[at..at + 3]) {
                *s += u64::from(*v);
            }
            count += 1;
        }
    }
    if count == 0 {
        let at = ((cy as usize).min(height - 1) * width + (cx as usize).min(width - 1)) * 4;
        return [rgba[at], rgba[at + 1], rgba[at + 2]];
    }
    sum.map(|s| ((s + count / 2) / count) as u8)
}

#[cfg(test)]
mod tests {
    use super::{Lay, Layers, PACE, disc_mean, lay};
    use std::time::{Duration, Instant};

    #[test]
    fn a_frame_dissolves_in_only_once_the_pace_allows() {
        let now = Instant::now();
        let mut l = Layers::default();
        assert_eq!(lay(&l, Some("a"), now), Lay::InPlace, "the first frame is laid as it comes");
        l.paths[0] = Some("a".into());
        assert_eq!(lay(&l, Some("a"), now), Lay::InPlace, "a sharper picture of the frame shown");
        assert_eq!(lay(&l, Some("b"), now), Lay::Dissolve);
        l.began = Some(now);
        let soon = now + Duration::from_millis(50);
        assert_eq!(lay(&l, Some("b"), soon), Lay::After(PACE - Duration::from_millis(50)), "the next waits for the pace");
        // Twin: once the pace has passed, the next one begins at once.
        assert_eq!(lay(&l, Some("b"), now + PACE), Lay::Dissolve);
        assert!(PACE * 2 > Duration::from_millis(1000) / 3, "two dissolves, a flash, take over a third of a second");
    }

    #[test]
    fn a_pick_reads_the_mean_of_its_own_disc_alone() {
        // A 20 x 10 picture: orange film at the left half, a blue picture at the right.
        let (w, h) = (20, 10);
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| if i % w < 10 { [200, 120, 60, 255] } else { [20, 40, 220, 255] }).collect();
        assert_eq!(disc_mean(&rgba, w, h, (0.2, 0.5), 3.0), [200, 120, 60], "a disc on the film reads the film");
        // Twin: the same disc on the picture reads the picture, and one across both reads neither.
        assert_eq!(disc_mean(&rgba, w, h, (0.8, 0.5), 3.0), [20, 40, 220]);
        let across = disc_mean(&rgba, w, h, (0.5, 0.5), 3.0);
        assert!(across != [200, 120, 60] && across != [20, 40, 220], "{across:?}");
        // A disc too small to hold a pixel's centre reads the pixel under it; one past the edge, its share.
        assert_eq!(disc_mean(&rgba, w, h, (0.01, 0.01), 0.1), [200, 120, 60]);
        assert_eq!(disc_mean(&rgba, w, h, (1.0, 1.0), 4.0), [20, 40, 220]);
    }
}
