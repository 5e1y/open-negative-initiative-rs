//! The measure's screens: each frame's channels and its negative, and the roll's histogram summed so
//! far with its ends; counted on the measure's thread, morphed here at the frame tick until they rest.

use super::super::{FRAME_TICK, SHELL};
use super::*;
use crate::display::Screen;
use crate::{CrtCurve, CrtEcho};
use oni_measure::roll::Measured;
use oni_settings::frame::RollReference;
use slint::{Model, Timer, TimerMode};
use std::collections::VecDeque;
use std::time::Duration;

/// The columns drawn: the editor's 1024 bins summed eight at a time.
pub(in crate::app) const BINS: usize = 128;
/// One frame's histogram becoming the next one's, eased in and out.
const MORPH: Duration = Duration::from_millis(320);
/// The frames before, kept as the phosphor's fading afterglow behind the one drawn.
const TRAIL: usize = 3;
/// The tallest bin's share of the screen's height, so a peak never touches the rim.
const HEADROOM: f32 = 0.9;

/// A histogram as a screen draws it: per bin the red, green and blue heights, 0 to 1, and for the
/// roll each channel's black and white on the same 0…1 axis.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct Drawn {
    pub bins: Vec<[f32; 3]>,
    pub ends: Option<[[f32; 2]; 3]>,
}

impl Drawn {
    /// The tile counted as the editor counts its picture, on the measure's thread.
    pub fn of(tile: &oni_measure::sheet::Picture, screen: Screen) -> Option<Drawn> {
        if tile.width == 0 || tile.height == 0 {
            return None;
        }
        let h = crate::histogram::measure(&tile.pixels, tile.width, tile.height, screen);
        Some(Drawn::stacked((0..3).map(|c| crate::histogram::coarsened(&h.channel(c), BINS)).collect(), None))
    }

    /// The roll so far, as `combined` reads it: its frames summed at its gains, its black and white.
    pub fn roll(frames: &[Measured], roll: &RollReference, mode: ConversionMode) -> Drawn {
        let sum = roll::summed(frames, roll, mode, BINS);
        let ends = [0, 1, 2].map(|c| [roll.black[c], roll.white[c]]);
        Drawn::stacked((0..3).map(|c| sum.iter().map(|b| b[c]).collect()).collect(), Some(ends))
    }

    /// Each channel's counts scaled to its own peak, as the editor's histogram scales them.
    fn stacked(channels: Vec<Vec<f32>>, ends: Option<[[f32; 2]; 3]>) -> Drawn {
        let heights: Vec<Vec<f32>> = channels.iter().map(|c| crate::histogram::heights(c)).collect();
        let n = heights.iter().map(Vec::len).min().unwrap_or(0);
        Drawn { bins: (0..n).map(|i| [heights[0][i], heights[1][i], heights[2][i]]).collect(), ends }
    }

    /// A flat histogram where `to`'s ends stand: the first one rises from it.
    fn floor(to: &Drawn) -> Drawn {
        Drawn { bins: vec![[0.0; 3]; to.bins.len()], ends: to.ends }
    }

    /// `k` of the way from this histogram to `to`.
    pub fn mixed(&self, to: &Drawn, k: f32) -> Drawn {
        let at = |a: f32, b: f32| a + (b - a) * k;
        let bins = to.bins.iter().enumerate().map(|(i, b)| self.bins.get(i).map_or(*b, |a| std::array::from_fn(|c| at(a[c], b[c])))).collect();
        let ends = match (self.ends, to.ends) {
            (Some(a), Some(b)) => Some(std::array::from_fn(|c| [at(a[c][0], b[c][0]), at(a[c][1], b[c][1])])),
            (_, b) => b,
        };
        Drawn { bins, ends }
    }
}

/// What one frame read sends: its histogram and negative, and the roll's once the frame voted.
pub(in crate::app) struct Landed {
    pub name: String,
    pub frame: Option<(Drawn, Bitmap)>,
    pub roll: Option<(Drawn, usize)>,
}

/// Cubic ease in and out, 0 to 1 over 0 to 1.
pub(in crate::app) fn eased(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

/// A channel's top edge in the screen's points, smoothed through the bins' midpoints, and the area
/// closed under it.
pub(in crate::app) fn shapes(bins: &[[f32; 3]], channel: usize, size: (f32, f32)) -> (String, String) {
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

/// Each channel's curve in a screen `size` points wide, its ends placed when it carries them.
fn curves(d: &Drawn, size: (f32, f32)) -> Vec<CrtCurve> {
    (0..3)
        .map(|c| {
            let (area, edge) = shapes(&d.bins, c, size);
            let [black, white] = d.ends.map_or([0.0; 2], |e| e[c]);
            CrtCurve { area: area.into(), edge: edge.into(), black: black.clamp(0.0, 1.0) * size.0, white: white.clamp(0.0, 1.0) * size.0 }
        })
        .collect()
}

/// A histogram on its way from the one shown to the one landed.
#[derive(Default)]
struct Morph {
    from: Option<Drawn>,
    to: Option<Drawn>,
}

impl Morph {
    fn shown(&self, k: f32) -> Option<Drawn> {
        let to = self.to.as_ref()?;
        Some(self.from.as_ref().map_or_else(|| to.clone(), |from| from.mixed(to, k)))
    }

    /// `to` landed: the one shown at `k` morphs into it, and the one it was becoming is returned.
    fn landed(&mut self, to: Drawn, k: f32) -> Option<Drawn> {
        self.from = Some(self.shown(k).unwrap_or_else(|| Drawn::floor(&to)));
        self.to.replace(to)
    }

    fn rest(&mut self) {
        self.from = None;
    }
}

/// What the measure's screens draw: the frame's and the roll's histograms morphing, the trail, the negative.
#[derive(Default)]
pub(in crate::app) struct Trace {
    frame: Morph,
    roll: Morph,
    since: Option<Instant>,
    trail: VecDeque<Drawn>,
    name: String,
    voted: usize,
    /// The two negatives cross-fading, and whether the second is the one shown.
    negatives: [Image; 2],
    second: bool,
    size: (f32, f32),
    roll_size: (f32, f32),
    timer: Timer,
}

impl Trace {
    fn k(&self, now: Instant) -> f32 {
        self.since.map_or(1.0, |at| eased(now.duration_since(at).as_secs_f32() / MORPH.as_secs_f32()))
    }

    fn moving(&self, now: Instant) -> bool {
        self.since.is_some_and(|at| now.duration_since(at) < MORPH)
    }
}

impl Ctl {
    /// A measure starting or landing: every screen dark, so a measure never opens on another's.
    pub(in crate::app) fn trace_cleared(&self) {
        {
            let mut s = self.state.borrow_mut();
            let t = &mut s.prepare.trace;
            t.timer.stop();
            let (size, roll_size) = (t.size, t.roll_size);
            *t = Trace { size, roll_size, ..Trace::default() };
        }
        self.publish_trace();
        self.publish_negatives();
    }

    /// A frame read: what it carries morphs in, the frame its histogram replaces joins the trail.
    pub(in crate::app) fn trace_landed(&self, landed: Landed) {
        let negative = {
            let mut s = self.state.borrow_mut();
            let t = &mut s.prepare.trace;
            let now = Instant::now();
            let k = t.k(now);
            if landed.frame.is_none() && landed.roll.is_none() {
                None
            } else {
                // Both histograms restart their morph together: the one not landing rests where it stands.
                match landed.frame {
                    Some((drawn, negative)) => {
                        if let Some(before) = t.frame.landed(drawn, k) {
                            t.trail.push_back(before);
                            while t.trail.len() > TRAIL {
                                t.trail.pop_front();
                            }
                        }
                        t.second = !t.second;
                        t.negatives[usize::from(t.second)] = picture(&negative);
                        t.name = landed.name;
                    }
                    None => t.frame.from = t.frame.shown(k),
                }
                match landed.roll {
                    Some((roll, voted)) => {
                        t.roll.landed(roll, k);
                        t.voted = voted;
                    }
                    None => t.roll.from = t.roll.shown(k),
                }
                t.since = Some(now);
                if !t.timer.running() {
                    t.timer.start(TimerMode::Repeated, FRAME_TICK, || {
                        if let Some(me) = SHELL.with(|s| s.borrow().upgrade()) {
                            me.publish_trace();
                        }
                    });
                }
                Some(())
            }
        };
        if negative.is_some() {
            self.publish_negatives();
        }
        self.publish_trace();
    }

    pub(in crate::app) fn trace_resized(&self, w: f32, h: f32) {
        self.state.borrow_mut().prepare.trace.size = (w, h);
        self.publish_trace();
    }

    pub(in crate::app) fn roll_trace_resized(&self, w: f32, h: f32) {
        self.state.borrow_mut().prepare.trace.roll_size = (w, h);
        self.publish_trace();
    }

    fn publish_negatives(&self) {
        let Some(w) = self.window() else { return };
        let s = self.state.borrow();
        let t = &s.prepare.trace;
        let p = w.global::<Preparing>();
        p.set_negative_a(t.negatives[0].clone());
        p.set_negative_b(t.negatives[1].clone());
        p.set_negative_second(t.second);
    }

    /// The screens as they stand this tick; the tick stops once they rest.
    pub(in crate::app) fn publish_trace(&self) {
        let Some(w) = self.window() else { return };
        let (frame, roll, trail, name, voted, size, roll_size, progress) = {
            let mut s = self.state.borrow_mut();
            let progress = s.prepare.progress;
            let t = &mut s.prepare.trace;
            let now = Instant::now();
            let k = t.k(now);
            if !t.moving(now) {
                t.timer.stop();
                t.since = None;
                t.frame.rest();
                t.roll.rest();
            }
            (t.frame.shown(k), t.roll.shown(k), t.trail.clone(), t.name.clone(), t.voted, t.size, t.roll_size, progress)
        };
        let p = w.global::<Preparing>();
        p.set_traced(frame.is_some());
        p.set_trace_name(if frame.is_some() { name.to_uppercase() } else { tr("Reading the first frame…").to_uppercase() }.into());
        p.set_trace_count(frame_line(progress).into());
        p.set_roll_voted(voted as i32);
        p.set_roll_line(if roll.is_some() { trn("ROLL · {n} FRAME", "ROLL · {n} FRAMES", voted as u64) } else { tr("ROLL").to_string() }.into());
        // Whole models: a screen reads its curve by index, which a row written in place does not wake.
        p.set_trace_curves(ModelRc::new(VecModel::from(frame.as_ref().map_or_else(Vec::new, |d| curves(d, size)))));
        p.set_roll_curves(ModelRc::new(VecModel::from(roll.as_ref().map_or_else(Vec::new, |d| curves(d, roll_size)))));
        let echoes: Vec<CrtEcho> = trail
            .iter()
            .enumerate()
            .flat_map(|(i, d)| {
                let fade = (i + 1) as f32 / trail.len() as f32;
                (0..3).map(move |c| CrtEcho { channel: c as i32, edge: shapes(&d.bins, c, size).1.into(), fade })
            })
            .collect();
        laid(&p.get_trace_trail(), echoes, |m| p.set_trace_trail(m));
    }
}

/// `LabScreen`'s place line: the frame read of the frames asked, both on two digits.
fn frame_line(progress: Option<(usize, usize)>) -> String {
    match progress {
        Some((done, total)) if done > 0 => tr!("FRAME {at}/{count}", at = format!("{done:02}"), count = format!("{total:02}")),
        Some((_, total)) => tr!("FRAME {at}/{count}", at = "--", count = format!("{total:02}")),
        None => tr("FRAME --/--").to_string(),
    }
}

/// Rows written into the kept model when its length holds, only those that changed, so the paths
/// drawn are kept, not made again.
fn laid<T: Clone + PartialEq + 'static>(model: &ModelRc<T>, rows: Vec<T>, set: impl FnOnce(ModelRc<T>)) {
    match model.as_any().downcast_ref::<VecModel<T>>().filter(|m| m.row_count() == rows.len()) {
        Some(kept) => rows.into_iter().enumerate().filter(|(i, r)| kept.row_data(*i).as_ref() != Some(r)).for_each(|(i, r)| kept.set_row_data(i, r)),
        None => set(ModelRc::new(VecModel::from(rows))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(v: f32, ends: Option<[[f32; 2]; 3]>) -> Drawn {
        Drawn { bins: vec![[v; 3]; BINS], ends }
    }

    #[test]
    fn the_morph_eases_from_one_histogram_to_the_next_and_rests_on_it() {
        assert_eq!((eased(0.0), eased(0.5), eased(1.0)), (0.0, 0.5, 1.0));
        assert!(eased(0.1) < 0.1 && eased(0.9) > 0.9, "slow at both ends");
        let (a, b) = (flat(0.2, Some([[0.1, 0.5]; 3])), flat(0.6, Some([[0.3, 0.9]; 3])));
        let mid = a.mixed(&b, 0.5);
        assert!((mid.bins[5][2] - 0.4).abs() < 1e-6);
        let ends = mid.ends.unwrap();
        assert!((ends[1][0] - 0.2).abs() < 1e-6 && (ends[1][1] - 0.7).abs() < 1e-6, "the ends move as the estimate settles: {ends:?}");
        assert_eq!(a.mixed(&b, 1.0), b);
        // Twin: a morph that jumped would already stand on the next histogram halfway.
        assert_ne!(mid, b);
    }

    #[test]
    fn a_tile_is_counted_into_each_channel_s_bins() {
        let tile = |v: f32| oni_measure::sheet::Picture { width: 40, height: 20, pixels: vec![[v, v, v, 1.0]; 800] };
        let d = Drawn::of(&tile(0.5), Screen::Srgb).unwrap();
        assert_eq!(d.bins.len(), BINS);
        let peak = (0..BINS).find(|&i| d.bins[i][1] == 1.0).unwrap();
        assert!(peak.abs_diff(BINS / 2) <= 1, "the grey's bin peaks: {peak}");
        assert_eq!((d.bins[BINS / 4][0], d.ends), (0.0, None));
        // Twin: another grey lands elsewhere.
        let dark = Drawn::of(&tile(0.2), Screen::Srgb).unwrap();
        assert_ne!(dark.bins, d.bins);
        assert!(Drawn::of(&oni_measure::sheet::Picture { width: 0, height: 0, pixels: Vec::new() }, Screen::Srgb).is_none());
    }

    #[test]
    fn the_shapes_span_the_screen_and_the_ends_stand_on_its_width() {
        let (area, edge) = shapes(&flat(1.0, None).bins, 0, (256.0, 100.0));
        assert!(edge.starts_with("M 0 10.00") && edge.ends_with("L 256.00 10.00"), "{edge}");
        assert!(area.starts_with(&edge) && area.ends_with("L 256.00 100.00 L 0 100.00 Z"));
        let roll = curves(&flat(0.5, Some([[0.25, 0.75], [0.0, 1.0], [-0.1, 1.2]])), (200.0, 50.0));
        assert_eq!(roll.iter().map(|c| (c.black, c.white)).collect::<Vec<_>>(), [(50.0, 150.0), (0.0, 200.0), (0.0, 200.0)]);
        // Twin: an empty screen draws nothing rather than a degenerate path.
        assert_eq!(shapes(&flat(1.0, None).bins, 0, (0.0, 100.0)), (String::new(), String::new()));
    }
}
