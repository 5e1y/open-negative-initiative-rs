//! The measure's histogram: each frame's counted off its tile on the measure's thread, then morphed
//! here from the frame before's into it at the frame tick, and held until the next one lands.

use super::super::{FRAME_TICK, SHELL};
use super::*;
use crate::display::Screen;
use crate::{Ink, TraceCurve};
use slint::{Model, Timer, TimerMode};
use std::collections::VecDeque;
use std::time::Duration;

/// The columns drawn: the editor's 1024 bins summed eight at a time.
pub(in crate::app) const BINS: usize = 128;
/// One frame's histogram becoming the next one's, eased in and out.
const MORPH: Duration = Duration::from_millis(320);
/// The frames before, kept as fading luminances behind the one drawn.
const TRAIL: usize = 3;
/// The tallest bin's share of the well's height, so a peak never touches the rim.
const HEADROOM: f32 = 0.9;
const LUMA: usize = 3;
/// Luminance first, under the three channels, as the editor's histogram lays them.
const CURVES: [(usize, Ink); 4] = [(LUMA, Ink::Primary), (0, Ink::Red), (1, Ink::Green), (2, Ink::Blue)];

/// One frame's histogram as drawn: per bin the red, green, blue and luminance heights, 0 to 1, and
/// where half its luminance lies, 0 to 1.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct Drawn {
    pub bins: Vec<[f32; 4]>,
    pub median: f32,
}

impl Drawn {
    /// The tile counted as the editor counts its picture, on the measure's thread.
    pub fn of(tile: &oni_measure::sheet::Picture, screen: Screen) -> Option<Drawn> {
        if tile.width == 0 || tile.height == 0 {
            return None;
        }
        let h = crate::histogram::measure(&tile.pixels, tile.width, tile.height, screen);
        let luma = crate::histogram::coarsened(&h.luma, BINS);
        let channels: Vec<Vec<f32>> = (0..3).map(|c| crate::histogram::heights(&crate::histogram::coarsened(&h.channel(c), BINS))).collect();
        let heights = crate::histogram::heights(&luma);
        let bins = (0..BINS).map(|i| [channels[0][i], channels[1][i], channels[2][i], heights[i]]).collect();
        Some(Drawn { bins, median: median(&luma) })
    }

    /// A flat histogram where `to`'s median stands: the first frame rises from it.
    fn floor(to: &Drawn) -> Drawn {
        Drawn { bins: vec![[0.0; 4]; to.bins.len()], median: to.median }
    }

    /// `k` of the way from this histogram to `to`.
    pub fn mixed(&self, to: &Drawn, k: f32) -> Drawn {
        let at = |a: f32, b: f32| a + (b - a) * k;
        let bins = to.bins.iter().enumerate().map(|(i, b)| self.bins.get(i).map_or(*b, |a| std::array::from_fn(|c| at(a[c], b[c])))).collect();
        Drawn { bins, median: at(self.median, to.median) }
    }
}

/// Where the cumulated counts pass half their total, 0 to 1; 0 for none.
pub(in crate::app) fn median(counts: &[f32]) -> f32 {
    let total: f32 = counts.iter().sum();
    if total <= 0.0 {
        return 0.0;
    }
    let mut run = 0.0;
    for (i, c) in counts.iter().enumerate() {
        if run + c >= total / 2.0 {
            let within = if *c > 0.0 { (total / 2.0 - run) / c } else { 0.0 };
            return (i as f32 + within) / counts.len() as f32;
        }
        run += c;
    }
    1.0
}

/// Cubic ease in and out, 0 to 1 over 0 to 1.
pub(in crate::app) fn eased(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

/// A channel's top edge in the well's points, smoothed through the bins' midpoints, and the area
/// closed under it.
pub(in crate::app) fn shapes(bins: &[[f32; 4]], channel: usize, size: (f32, f32)) -> (String, String) {
    use std::fmt::Write;
    let (w, h) = size;
    let n = bins.len();
    if n < 2 || w <= 0.0 || h <= 0.0 {
        return (String::new(), String::new());
    }
    let point = |i: usize| ((i as f32 + 0.5) / n as f32 * w, h - bins[i][channel].clamp(0.0, 1.0) * h * HEADROOM);
    let mut edge = String::with_capacity(n * 24);
    let (_, y0) = point(0);
    let _ = write!(edge, "M 0 {y0:.2}");
    for i in 0..n - 1 {
        let ((x, y), (nx, ny)) = (point(i), point(i + 1));
        let _ = write!(edge, " Q {x:.2} {y:.2} {:.2} {:.2}", (x + nx) / 2.0, (y + ny) / 2.0);
    }
    let (_, yn) = point(n - 1);
    let _ = write!(edge, " L {w:.2} {yn:.2}");
    let area = format!("{edge} L {w:.2} {h:.2} L 0 {h:.2} Z");
    (area, edge)
}

/// What the measure's screen draws: the histogram shown, the one it is becoming, and the trail.
#[derive(Default)]
pub(in crate::app) struct Trace {
    from: Option<Drawn>,
    to: Option<Drawn>,
    since: Option<Instant>,
    trail: VecDeque<Drawn>,
    name: String,
    size: (f32, f32),
    timer: Timer,
}

impl Trace {
    /// The histogram drawn now.
    fn shown(&self, now: Instant) -> Option<Drawn> {
        let to = self.to.as_ref()?;
        let k = self.since.map_or(1.0, |at| eased(now.duration_since(at).as_secs_f32() / MORPH.as_secs_f32()));
        Some(self.from.as_ref().map_or_else(|| to.clone(), |from| from.mixed(to, k)))
    }

    fn moving(&self, now: Instant) -> bool {
        self.since.is_some_and(|at| now.duration_since(at) < MORPH)
    }
}

impl Ctl {
    /// A measure starting or landing: nothing drawn, so a measure never opens on another's histogram.
    pub(in crate::app) fn trace_cleared(&self) {
        {
            let mut s = self.state.borrow_mut();
            let t = &mut s.prepare.trace;
            t.timer.stop();
            let size = t.size;
            *t = Trace { size, ..Trace::default() };
        }
        self.publish_trace();
    }

    /// A frame's histogram landed: the one drawn now morphs into it, the one it was becoming joins the trail.
    pub(in crate::app) fn trace_landed(&self, name: String, drawn: Drawn) {
        {
            let mut s = self.state.borrow_mut();
            let t = &mut s.prepare.trace;
            let now = Instant::now();
            let from = t.shown(now).unwrap_or_else(|| Drawn::floor(&drawn));
            if let Some(before) = t.to.take() {
                t.trail.push_back(before);
                while t.trail.len() > TRAIL {
                    t.trail.pop_front();
                }
            }
            t.from = Some(from);
            t.to = Some(drawn);
            t.since = Some(now);
            t.name = name;
            if !t.timer.running() {
                t.timer.start(TimerMode::Repeated, FRAME_TICK, || {
                    if let Some(me) = SHELL.with(|s| s.borrow().upgrade()) {
                        me.publish_trace();
                    }
                });
            }
        }
        self.publish_trace();
    }

    pub(in crate::app) fn trace_resized(&self, w: f32, h: f32) {
        self.state.borrow_mut().prepare.trace.size = (w, h);
        self.publish_trace();
    }

    /// The histogram as it stands this tick; the tick stops once it rests.
    pub(in crate::app) fn publish_trace(&self) {
        let Some(w) = self.window() else { return };
        let (shown, trail, name, size, count) = {
            let mut s = self.state.borrow_mut();
            let count = s.prepare.progress.map(|(done, total)| format!("{done} / {total}")).unwrap_or_default();
            let t = &mut s.prepare.trace;
            let now = Instant::now();
            if !t.moving(now) {
                t.timer.stop();
                t.since = None;
            }
            (t.shown(now), t.trail.clone(), t.name.clone(), t.size, count)
        };
        let p = w.global::<Preparing>();
        p.set_traced(shown.is_some());
        p.set_trace_name(name.into());
        p.set_trace_count(count.into());
        p.set_trace_marker(shown.as_ref().map_or(0.0, |d| d.median * size.0));
        let curves: Vec<TraceCurve> = shown
            .iter()
            .flat_map(|d| {
                CURVES.into_iter().map(move |(c, ink)| {
                    let (area, edge) = shapes(&d.bins, c, size);
                    TraceCurve { area: area.into(), edge: edge.into(), ink }
                })
            })
            .collect();
        laid(&p.get_trace_curves(), curves, |m| p.set_trace_curves(m));
        let edges: Vec<SharedString> = trail.iter().map(|d| shapes(&d.bins, LUMA, size).1.into()).collect();
        laid(&p.get_trace_trail(), edges, |m| p.set_trace_trail(m));
    }
}

/// Rows written into the kept model when its length holds, so the paths drawn are kept, not made again.
fn laid<T: Clone + 'static>(model: &ModelRc<T>, rows: Vec<T>, set: impl FnOnce(ModelRc<T>)) {
    match model.as_any().downcast_ref::<VecModel<T>>().filter(|m| m.row_count() == rows.len()) {
        Some(kept) => rows.into_iter().enumerate().for_each(|(i, r)| kept.set_row_data(i, r)),
        None => set(ModelRc::new(VecModel::from(rows))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(v: f32, median: f32) -> Drawn {
        Drawn { bins: vec![[v; 4]; BINS], median }
    }

    #[test]
    fn the_morph_eases_from_one_histogram_to_the_next_and_rests_on_it() {
        assert_eq!((eased(0.0), eased(0.5), eased(1.0)), (0.0, 0.5, 1.0));
        assert!(eased(0.1) < 0.1 && eased(0.9) > 0.9, "slow at both ends");
        let (a, b) = (flat(0.2, 0.3), flat(0.6, 0.7));
        let mid = a.mixed(&b, 0.5);
        assert!((mid.bins[5][2] - 0.4).abs() < 1e-6 && (mid.median - 0.5).abs() < 1e-6);
        assert_eq!(a.mixed(&b, 1.0), b);
        // Twin: a morph that jumped would already stand on the next histogram halfway.
        assert_ne!(mid, b);
    }

    #[test]
    fn a_tile_is_counted_into_its_bins_and_its_median() {
        let tile = |v: f32| oni_measure::sheet::Picture { width: 40, height: 20, pixels: vec![[v, v, v, 1.0]; 800] };
        let d = Drawn::of(&tile(0.5), Screen::Srgb).unwrap();
        assert_eq!(d.bins.len(), BINS);
        let peak = (0..BINS).find(|&i| d.bins[i][LUMA] == 1.0).unwrap();
        assert!(peak.abs_diff(BINS / 2) <= 1, "the grey's bin peaks: {peak}");
        assert_eq!(d.bins[BINS / 4][0], 0.0);
        assert!((d.median - 0.5).abs() < 2.0 / BINS as f32, "{}", d.median);
        // Twin: another grey lands elsewhere.
        let dark = Drawn::of(&tile(0.2), Screen::Srgb).unwrap();
        assert_ne!(dark.bins, d.bins);
        assert!(dark.median < 0.3);
        assert!(Drawn::of(&oni_measure::sheet::Picture { width: 0, height: 0, pixels: Vec::new() }, Screen::Srgb).is_none());
    }

    #[test]
    fn the_shapes_span_the_well_and_close_under_the_edge() {
        let (area, edge) = shapes(&flat(1.0, 0.5).bins, LUMA, (256.0, 100.0));
        assert!(edge.starts_with("M 0 10.00") && edge.ends_with("L 256.00 10.00"), "{edge}");
        assert!(area.starts_with(&edge) && area.ends_with("L 256.00 100.00 L 0 100.00 Z"));
        // Twin: an empty well draws nothing rather than a degenerate path.
        assert_eq!(shapes(&flat(1.0, 0.5).bins, LUMA, (0.0, 100.0)), (String::new(), String::new()));
        assert_eq!(median(&[0.0, 2.0, 0.0, 0.0]), 0.375);
        assert_eq!(median(&[0.0; 4]), 0.0);
    }
}
