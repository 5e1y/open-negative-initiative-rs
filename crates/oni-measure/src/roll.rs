//! `RollReference`'s measuring: each frame's own Classic, then the roll from its frames, so a light
//! leak or a fogged first frame never sets the roll's range; and the frames Prepare the Roll writes.

use crate::auto_levels::{self, Balance};
use crate::histogram::Histogram;
use oni_core::parameters;
use oni_settings::cg::Rect;
use oni_settings::frame::{FlatField, RollReference};
use oni_settings::{ConversionMode, Id, PipelineSettings};

/// The base pick's disc at rest, in pixels of the measuring copy: the pipette's narrowest, since the
/// bare film can be a border a few pixels wide.
pub const BASE_RADIUS: f64 = crate::pipette::RADIUS_RANGE.0;

/// The preference the base step's Picker Size is remembered under, in no sidecar or preset.
pub const BASE_RADIUS_KEY: &str = "rollBaseRadius";

/// One frame's part in the roll: its own Classic, and the raw tails its ends were placed on.
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    pub path: String,
    pub balance: Balance,
    /// Each channel's low and high tails at `balance.stops`, before any clamp to the window.
    pub tails: [[f32; 2]; 3],
    /// Each channel's median at `balance.stops`, off the same histogram as the tails.
    pub median: Option<[f32; 3]>,
    /// That histogram's counts, each bin at its index over the last on the counted 0…1 window.
    pub counts: Vec<[f32; 3]>,
}

/// `RollReference.measured`: a frame's Classic through `reading`, which renders it at given gains.
pub fn measured(path: &str, settings: &PipelineSettings, mut reading: impl FnMut([f32; 3]) -> Histogram) -> Option<Measured> {
    let stops = settings.gains.stops;
    let mode = settings.mode;
    let first = reading(stops);
    if first.is_empty() {
        return None;
    }
    let moved = auto_levels::recentred(&first, stops, mode, 0.0)?;
    let after = if moved == stops { first } else { reading(moved) };
    let tails = after.ends.as_ref().filter(|e| e.len() == 3).map(|e| [e[0], e[1], e[2]])?;
    let levels = auto_levels::placement(&after, 0.0)?;
    let shift = [0, 1, 2].map(|c| auto_levels::shift(moved[c] - stops[c], mode));
    let scale = (after.rgb.len().saturating_sub(1)).max(1) as f32;
    let medians: Vec<f32> = (0..3).filter_map(|c| auto_levels::median(&after.channel(c)).map(|m| m / scale)).collect();
    let median = (medians.len() == 3).then(|| [medians[0], medians[1], medians[2]]);
    Some(Measured { path: path.to_string(), balance: Balance { stops: moved, levels, shift }, tails, median, counts: after.rgb })
}

/// The roll's histogram as `combined` reads it: every frame's counts moved to the roll's gains, as
/// its tails are, and summed into `bins` over 0…1; a count moved past either end is dropped.
pub fn summed(frames: &[Measured], roll: &RollReference, mode: ConversionMode, bins: usize) -> Vec<[f32; 3]> {
    let mut out = vec![[0.0f32; 3]; bins];
    if bins == 0 {
        return out;
    }
    for f in frames {
        let last = f.counts.len().saturating_sub(1).max(1) as f32;
        for c in 0..3 {
            let s = auto_levels::shift(roll.stops[c] - f.balance.stops[c], mode);
            for (i, b) in f.counts.iter().enumerate() {
                let x = i as f32 / last + s;
                if (0.0..=1.0).contains(&x) {
                    out[((x * bins as f32) as usize).min(bins - 1)][c] += b[c];
                }
            }
        }
    }
    out
}

fn middle(values: &mut [f32]) -> f32 {
    values.sort_by(|a, b| a.total_cmp(b));
    let m = values.len() / 2;
    if values.len().is_multiple_of(2) { (values[m - 1] + values[m]) / 2.0 } else { values[m] }
}

/// `RollReference.combined`: gains on the median, each end on the second most extreme frame from
/// three frames up; a picked base is every channel's black, moved to the roll's gains.
pub fn combined(frames: &[Measured], mode: ConversionMode, base: Option<[f32; 3]>) -> Option<RollReference> {
    if frames.is_empty() {
        return None;
    }
    let stops = [0, 1, 2].map(|c| middle(&mut frames.iter().map(|f| f.balance.stops[c]).collect::<Vec<_>>()));
    // A gain is an exact translation of the densities, so each frame's tails move to the roll's gains unread.
    let moved: Vec<[[f32; 2]; 3]> = frames
        .iter()
        .map(|f| {
            [0, 1, 2].map(|c| {
                let s = auto_levels::shift(stops[c] - f.balance.stops[c], mode);
                [f.tails[c][0] + s, f.tails[c][1] + s]
            })
        })
        .collect();
    let dropped = usize::from(frames.len() >= 3);
    let mut black = [0, 1, 2].map(|c| {
        let mut lows: Vec<f32> = moved.iter().map(|m| m[c][0]).collect();
        lows.sort_by(|a, b| a.total_cmp(b));
        lows[dropped]
    });
    let highs = [0, 1, 2].map(|c| {
        let mut his: Vec<f32> = moved.iter().map(|m| m[c][1]).collect();
        his.sort_by(|a, b| b.total_cmp(a));
        his[dropped]
    });
    let shifted = base.map(|picked| [0, 1, 2].map(|c| picked[c] + auto_levels::shift(stops[c], mode)));
    // Each channel's black on the bare film itself: the scene's own zero, neutral by construction.
    if let Some(s) = shifted {
        black = s;
    }
    let mut white = [0.0f32; 3];
    for c in 0..3 {
        black[c] = black[c].max(0.0).min(1.0 - auto_levels::MINIMUM_SPAN);
        white[c] = highs[c].max(black[c] + auto_levels::MINIMUM_SPAN).min(1.0);
    }
    // Read only when every frame carried its own, moved to the roll's gains as the tails are.
    let median = frames.iter().all(|f| f.median.is_some()).then(|| {
        [0, 1, 2].map(|c| {
            middle(
                &mut frames
                    .iter()
                    .map(|f| f.median.expect("checked above")[c] + auto_levels::shift(stops[c] - f.balance.stops[c], mode))
                    .collect::<Vec<_>>(),
            )
        })
    });
    Some(RollReference { id: Id::new(), stops, black, white, base: shifted, median })
}

/// The settings a frame of the roll is measured from: the look at rest, framing, flat-field and dust
/// carried over, on the roll's film, window and mask.
pub fn start(before: &PipelineSettings, film: ConversionMode, window: Rect, mask: Option<&FlatField>, capture_stops: f32) -> PipelineSettings {
    let mut s = parameters::carried(&parameters::imported(before), film);
    s.corrections = before.corrections.clone();
    s.capture_stops = capture_stops;
    s.primaries_converted = false;
    s.geometry.measure_window = Some(window);
    if let Some(m) = mask {
        s.flat_field = m.clone();
    }
    s
}

/// The settings a slide of the roll is levelled from, on no roll: the look and any earlier roll at
/// rest, framing, flat-field and dust carried over, on the roll's window and mask.
pub fn slide(before: &PipelineSettings, window: Rect, mask: Option<&FlatField>, capture_stops: f32) -> PipelineSettings {
    let mut bare = before.clone();
    bare.roll_reference = None;
    let mut s = parameters::carried(&parameters::imported(&bare), ConversionMode::Positive);
    s.corrections = before.corrections.clone();
    s.capture_stops = capture_stops;
    s.primaries_converted = false;
    s.geometry.measure_window = Some(window);
    if let Some(m) = mask {
        s.flat_field = m.clone();
    }
    s
}

/// One frame Prepare the Roll was asked to read; a flagged frame joins the roll without voting.
#[derive(Clone, Debug)]
pub struct Job {
    pub path: String,
    pub before: PipelineSettings,
    pub start: PipelineSettings,
    pub flagged: bool,
}

/// What a roll's measuring writes: the reference and every frame's settings before and after.
#[derive(Clone, Debug)]
pub struct Written {
    pub reference: RollReference,
    pub frames: Vec<(String, PipelineSettings, PipelineSettings)>,
}

/// `measureRoll` without its sheet: each unflagged frame measured, the roll combined, every frame
/// written onto it; a frame that reads nothing is left out, and no measured frame writes nothing.
pub fn measure_roll(
    jobs: &[Job],
    film: ConversionMode,
    base: Option<[f32; 3]>,
    mut measure: impl FnMut(&Job) -> Option<Measured>,
) -> Option<Written> {
    let mut measured = Vec::new();
    let mut kept = Vec::new();
    for job in jobs {
        if job.flagged {
            kept.push(job);
        } else if let Some(frame) = measure(job) {
            measured.push(frame);
            kept.push(job);
        }
    }
    let reference = combined(&measured, film, base)?;
    let frames = kept
        .iter()
        .map(|j| (j.path.clone(), j.before.clone(), oni_core::balance::written(&reference, &j.start)))
        .collect();
    Some(Written { reference, frames })
}
