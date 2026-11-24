//! The five-point curve's gestures, the saturation's: a press takes the point under it,
//! else the closest along the tones, and a double click's second release returns it to its rest.

use super::geometry::{Size, distance, ds};
use super::{Edit, Gestures, Held, Plot, Then};
use oni_core::EditorStore;
use oni_settings::PipelineSettings;
use oni_settings::cg::Point;
use std::time::Instant;

/// A curve of five points, its ends held on their tone, laid across the plot at `tone^GAMMA`.
pub trait Points: Clone {
    const GAMMA: f64;
    fn points(&self) -> &[Point];
    fn points_mut(&mut self) -> &mut [Point];
    fn move_point(&mut self, index: usize, point: Point);
    /// Where a double click returns point `index`.
    fn rested(&self, index: usize) -> Point;
}

pub fn screen<C: Points>(p: Point, size: Size) -> (f64, f64) {
    (p.x.powf(C::GAMMA) * size.w, (1.0 - p.y) * size.h)
}

/// The point under the pointer, within a grab area: the closest, the first of a tie.
pub fn nearest<C: Points>(curve: &C, at: (f64, f64), size: Size) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, p) in curve.points().iter().enumerate() {
        let d = distance(screen::<C>(*p, size), at);
        if d < ds::GRAB_SIZE && best.is_none_or(|(_, b)| d < b) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// The point closest along the tones, whatever its height.
pub fn closest_along<C: Points>(curve: &C, at: (f64, f64), size: Size) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, p) in curve.points().iter().enumerate() {
        let d = (screen::<C>(*p, size).0 - at.0).abs();
        if best.is_none_or(|(_, b)| d < b) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// Where the pointer asks a point to be: across read back on the curve's tones, up its value.
pub fn asked<C: Points>(at: (f64, f64), size: Size) -> Point {
    let across = (at.0 / size.w.max(1.0)).clamp(0.0, 1.0);
    Point::new(across.powf(1.0 / C::GAMMA), 1.0 - at.1 / size.h.max(1.0))
}

/// The view's memory of the last two presses: a double click's first press has already moved a
/// point to the pointer, so the return starts from before it.
#[derive(Clone, Debug, Default)]
pub struct Presses {
    pub dragged: Option<usize>,
    before_press: Vec<Point>,
    before_last_press: Vec<Point>,
}

impl Presses {
    /// A press takes its point and lays it at the pointer at once.
    pub fn press<C: Points>(&mut self, curve: &mut C, at: (f64, f64), size: Size) {
        self.before_last_press = std::mem::take(&mut self.before_press);
        self.before_press = curve.points().to_vec();
        self.dragged = nearest(curve, at, size).or_else(|| closest_along(curve, at, size));
        self.drag(curve, at, size);
    }

    pub fn drag<C: Points>(&self, curve: &mut C, at: (f64, f64), size: Size) {
        if let Some(index) = self.dragged {
            curve.move_point(index, asked::<C>(at, size));
        }
    }

    pub fn release(&mut self) {
        self.dragged = None;
    }

    /// The double click's end: the points before its first press, the one under the pointer at its rest.
    pub fn double<C: Points>(&self, curve: &C, at: (f64, f64), size: Size) -> Option<C> {
        let index = nearest(curve, at, size)?;
        let mut next = curve.clone();
        if self.before_last_press.len() == next.points().len() {
            next.points_mut().copy_from_slice(&self.before_last_press);
        }
        let rest = next.rested(index);
        next.move_point(index, rest);
        Some(next)
    }
}

impl Gestures {
    /// A press on a five-point plot: its point at the pointer at once, written at the cadence.
    pub(super) fn points_press(&mut self, store: &mut EditorStore, plot: Plot, view: &PipelineSettings, at: (f64, f64), now: Instant) -> Then {
        let size = self.sizes[plot as usize];
        let double = self.double(plot as usize, now);
        let mut curve = view.saturation.clone();
        self.saturation.press(&mut curve, at, size);
        self.begin(store, Held::Points { double, at }, Edit::Saturation(curve));
        self.offer(store, now)
    }

    /// The held point at the pointer; `None` off the plot that holds it.
    pub(super) fn points_drag(&mut self, plot: Plot, edit: Edit, at: (f64, f64)) -> Option<Edit> {
        let size = self.sizes[plot as usize];
        match edit {
            Edit::Saturation(mut c) if plot == Plot::Saturation => {
                self.saturation.drag(&mut c, at, size);
                Some(Edit::Saturation(c))
            }
            _ => None,
        }
    }

    /// A double click's second release: the point under it back at its rest.
    pub(super) fn points_double(&mut self, at: (f64, f64)) {
        let next = match &self.live {
            Some(Edit::Saturation(c)) => self.saturation.double(c, at, self.sizes[Plot::Saturation as usize]).map(Edit::Saturation),
            _ => None,
        };
        if let Some(next) = next {
            self.live = Some(next);
        }
    }

    pub(super) fn points_released(&mut self) {
        self.saturation.release();
    }

    /// The point under the hand on `plot`, drawn lit.
    pub fn points_dragged(&self, plot: Plot) -> Option<usize> {
        if !matches!(self.held, Some(Held::Points { .. })) {
            return None;
        }
        match (plot, &self.live) {
            (Plot::Saturation, Some(Edit::Saturation(_))) => self.saturation.dragged,
            _ => None,
        }
    }
}
