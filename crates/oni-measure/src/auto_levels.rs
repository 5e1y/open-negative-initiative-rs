//! `AutoLevels`: where the information in a channel begins and ends, placed in two steps, the gains
//! first, then each channel's ends at those gains, read off a measured histogram.

use crate::histogram::Histogram;
use oni_core::balance::{self, Suggestion};
use oni_settings::axis::graduation;
use oni_settings::blocks::Gains;
use oni_settings::levels::LevelsChannel;
use oni_settings::{ConversionMode, PipelineSettings};

/// The share a check's synthetic histogram skips at each end; a reading of the app skips nothing.
pub const DEFAULT_THRESHOLD: f32 = 0.001;

/// The narrowest span a suggestion may propose, a share of the measured window: below it the
/// channel is flat and the honest answer is none.
pub const MINIMUM_SPAN: f32 = 0.02;

/// Green, the channel the other two are brought onto.
pub const REFERENCE: usize = 1;

/// `Method.classic`, the one placement, by the name a sidecar stores.
pub const CLASSIC: &str = "classic";

fn sum(counts: &[f32]) -> f32 {
    counts.iter().fold(0.0, |a, b| a + b)
}

/// The first bin past `threshold` of the channel from each end; `>`, so a threshold of 0 lands on
/// the first bin holding a pixel.
pub fn suggest_counts(counts: &[f32], threshold: f32) -> Option<Suggestion> {
    if counts.len() <= 1 {
        return None;
    }
    let total = sum(counts);
    if !(total > 0.0) {
        return None;
    }
    let allowed = total * threshold.max(0.0);
    let mut running = 0.0f32;
    let mut low = counts.len() - 1;
    for (bin, &count) in counts.iter().enumerate() {
        running += count;
        if running > allowed {
            low = bin;
            break;
        }
    }
    running = 0.0;
    let mut high = 0;
    for (offset, &count) in counts.iter().rev().enumerate() {
        running += count;
        if running > allowed {
            high = counts.len() - 1 - offset;
            break;
        }
    }
    if high <= low {
        return None;
    }
    let scale = (counts.len() - 1) as f32;
    let s = Suggestion::ends(low as f32 / scale, high as f32 / scale);
    (s.white - s.black >= MINIMUM_SPAN).then_some(s)
}

/// One channel's ends, the reading's own ends in place of the counts' where it carries them.
pub fn suggest(h: &Histogram, channel: usize, threshold: f32) -> Option<Suggestion> {
    if h.is_empty() || channel > 2 {
        return None;
    }
    let mut out = suggest_counts(&h.channel(channel), threshold)?;
    if let Some(tails) = h.ends.as_ref().and_then(|e| e.get(channel)) {
        out.black = tails[0].max(0.0).min(1.0 - MINIMUM_SPAN);
        out.white = tails[1].max(out.black + MINIMUM_SPAN).min(1.0);
    }
    Some(out)
}

/// Classic: every channel's ends, or none when one channel has none.
pub fn placement(h: &Histogram, threshold: f32) -> Option<[Suggestion; 3]> {
    if h.is_empty() {
        return None;
    }
    Some([suggest(h, 0, threshold)?, suggest(h, 1, threshold)?, suggest(h, 2, threshold)?])
}

/// One pair of ends on the RGB composite, the three channels' counts summed, laid on every channel;
/// the reading's outermost ends in place of the counts' where it carries them.
pub fn composite(h: &Histogram, threshold: f32) -> Option<[Suggestion; 3]> {
    if h.is_empty() {
        return None;
    }
    let summed: Vec<f32> = h.rgb.iter().map(|b| b[0] + b[1] + b[2]).collect();
    let mut out = suggest_counts(&summed, threshold)?;
    if let Some(e) = h.ends.as_ref().filter(|e| e.len() == 3) {
        let low = e.iter().map(|t| t[0]).fold(f32::INFINITY, f32::min);
        let high = e.iter().map(|t| t[1]).fold(f32::NEG_INFINITY, f32::max);
        out.black = low.max(0.0).min(1.0 - MINIMUM_SPAN);
        out.white = high.max(out.black + MINIMUM_SPAN).min(1.0);
    }
    Some([out; 3])
}

/// The bin, fractional, below which `fraction` of the counts lie.
pub fn quantile(counts: &[f32], fraction: f32) -> Option<f32> {
    let total = sum(counts);
    if !(total > 0.0) {
        return None;
    }
    let mark = total * fraction.clamp(0.0, 1.0);
    let mut running = 0.0f32;
    for (bin, &count) in counts.iter().enumerate() {
        let next = running + count;
        if next >= mark {
            if !(count > 0.0) {
                return Some(bin as f32);
            }
            return Some(bin as f32 + (mark - running) / count);
        }
        running = next;
    }
    Some((counts.len() - 1) as f32)
}

pub fn median(counts: &[f32]) -> Option<f32> {
    quantile(counts, 0.5)
}

/// Stops of stage-4 gain worth the whole measured window: its density span times stops per decade.
pub fn stops_per_unit_e() -> f32 {
    graduation::span() * 10.0f32.log2()
}

/// What a gain does to the measurement: a translation of the density axis, the pedestal sitting
/// upstream of the gain. The only place the mode's sign appears.
pub fn shift(stops: f32, mode: ConversionMode) -> f32 {
    (if mode.inverts_gain_sense() { -stops } else { stops }) / stops_per_unit_e()
}

pub fn stops_for_shift(shift: f32, mode: ConversionMode) -> f32 {
    (if mode.inverts_gain_sense() { -shift } else { shift }) * stops_per_unit_e()
}

/// Step one: red and blue brought onto green's median, never past the window's ends, held to the
/// gains' track; a channel too flat to place still moves, being the one needing it.
pub fn recentred(h: &Histogram, stops: [f32; 3], mode: ConversionMode, threshold: f32) -> Option<[f32; 3]> {
    if h.is_empty() {
        return None;
    }
    let scale = (h.rgb.len() - 1) as f32;
    let channels: Vec<Vec<f32>> = (0..3).map(|c| h.channel(c)).collect();
    let target = median(&channels[REFERENCE])? / scale;
    let mut out = stops;
    for c in (0..3).filter(|&c| c != REFERENCE) {
        let Some(bin) = median(&channels[c]) else { continue };
        let mut travel = target - bin / scale;
        if let Some(ends) = suggest_counts(&channels[c], threshold) {
            travel = travel.max(-ends.black).min(1.0 - ends.white);
        }
        let asked = stops[c] + stops_for_shift(travel, mode);
        out[c] = asked.max(Gains::RANGE.0).min(Gains::RANGE.1);
    }
    Some(out)
}

/// The two steps as one value, with the share of the window step one moved each channel.
#[derive(Clone, Debug, PartialEq)]
pub struct Balance {
    pub stops: [f32; 3],
    pub levels: [Suggestion; 3],
    pub shift: [f32; 3],
}

impl Balance {
    /// The handles as the pre-step-one histogram would show them.
    pub fn shown(&self, channel: usize, mode: ConversionMode) -> oni_settings::levels::Levels {
        let mut out = self.levels[channel];
        let travel = self.shift[channel];
        out.black = (out.black - travel).max(0.0).min(1.0 - MINIMUM_SPAN);
        out.white = (out.white - travel).max(out.black + MINIMUM_SPAN).min(1.0);
        balance::placed(&out, mode)
    }

    pub fn core(&self) -> balance::Balance {
        balance::Balance { stops: self.stops, levels: self.levels }
    }
}

/// `AutoLevels.balance`: re-measured after the gains, which translate the density axis exactly
/// but not the mass they carry over the window's ends.
pub fn two_steps(
    h: &Histogram,
    stops: [f32; 3],
    mode: ConversionMode,
    threshold: f32,
    mut remeasuring: impl FnMut([f32; 3]) -> Histogram,
) -> Option<Balance> {
    let moved = recentred(h, stops, mode, threshold)?;
    let shift = [0, 1, 2].map(|c| self::shift(moved[c] - stops[c], mode));
    let after = if moved == stops { h.clone() } else { remeasuring(moved) };
    let levels = placement(&after, threshold)?;
    Some(Balance { stops: moved, levels, shift })
}

/// The Auto levels key on a negative or a black and white frame: Classic's two steps from its own
/// gains. A slide's key reads its cells together (`key::slide_read`).
pub fn keyed(stops: [f32; 3], mode: ConversionMode, mut reading: impl FnMut([f32; 3]) -> Histogram) -> Option<Balance> {
    let first = reading(stops);
    if first.is_empty() { None } else { two_steps(&first, stops, mode, 0.0, reading) }
}

/// The tones a slide's chroma is read at, from its darkest cell to its lightest, and the share of that
/// span each band takes on either side: the shadows' and the lights' greys, clear of the clipped ends.
pub const CHROMA_MARKS: [f32; 2] = [0.10, 0.90];
pub const CHROMA_BAND: f32 = 0.05;
/// The fewest cells a band reads its colour on, the nearest tones taken when the band holds fewer.
pub const BAND_CELLS: usize = 5;
/// The midtones a slide's grey is snapped on, the quarter of them nearest grey, and how far from grey
/// those may stand before the scene is taken for colour rather than a cast (Auto Color's own rule).
pub const MID_TONES: [f32; 2] = [0.30, 0.70];
pub const MID_KEPT: f32 = 0.25;
pub const NEUTRAL_CHROMA: f32 = 0.08;

/// A slide's points: its cells at 10 % and 90 % of its tones brought to grey, three channels read
/// together, one black and one white on the aligned channels' ends, its near-grey midtones then grey too.
pub fn slide_points(h: &Histogram, cells: &[[f32; 3]], luma: &[f32]) -> Option<[Suggestion; 3]> {
    let ends = placement(h, 0.0)?;
    if cells.len() != luma.len() || cells.is_empty() {
        return None;
    }
    let lo = luma.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = luma.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let median = |values: Vec<f32>| -> f32 {
        let mut v = values;
        v.sort_by(f32::total_cmp);
        v[v.len() / 2]
    };
    let band = |mark: f32| -> [f32; 3] {
        let target = lo + mark * (hi - lo);
        let mut near: Vec<usize> = (0..cells.len()).collect();
        near.sort_by(|a, b| (luma[*a] - target).abs().total_cmp(&(luma[*b] - target).abs()));
        let inside = near.iter().take_while(|&&i| (luma[i] - target).abs() <= CHROMA_BAND * (hi - lo)).count();
        let held = &near[..inside.max(BAND_CELLS).min(near.len())];
        [0, 1, 2].map(|c| median(held.iter().map(|&i| cells[i][c]).collect()))
    };
    let (dark, light) = (band(CHROMA_MARKS[0]), band(CHROMA_MARKS[1]));
    let marks: [[f32; 2]; 3] = [0, 1, 2].map(|c| [dark[c], light[c]]);
    if marks.iter().any(|m| !(m[1] - m[0] >= MINIMUM_SPAN)) {
        return None;
    }
    let [low, high] = marks[REFERENCE];
    let aligned = |c: usize, x: f32| low + (x - marks[c][0]) * (high - low) / (marks[c][1] - marks[c][0]);
    let back = |c: usize, y: f32| marks[c][0] + (y - low) * (marks[c][1] - marks[c][0]) / (high - low);
    let black = (0..3).map(|c| aligned(c, ends[c].black)).fold(f32::INFINITY, f32::min);
    let white = (0..3).map(|c| aligned(c, ends[c].white)).fold(f32::NEG_INFINITY, f32::max);
    // Unbounded: a channel whose black lands past the counted window keeps it there, or its shadows
    // fall darker than the others' and the grey goes.
    let mut points = [0, 1, 2].map(|c| {
        let b = back(c, black);
        Suggestion { black: b, white: back(c, white).max(b + MINIMUM_SPAN), ..ends[c] }
    });
    // Green's window lays the greys' targets; red and blue are then solved to land on them at all three tones.
    if let Some(mid) = neutral_mid(cells, luma, (lo, hi), &points) {
        let g = &points[REFERENCE];
        let on_green = |x: f32| ((x - g.black) / (g.white - g.black)).clamp(1e-3, 1.0 - 1e-3);
        let targets = [on_green(dark[REFERENCE]), on_green(mid[REFERENCE]), on_green(light[REFERENCE])];
        for c in (0..3).filter(|&c| c != REFERENCE) {
            if let Some((black, white, m)) = three_point([dark[c], mid[c], light[c]], targets) {
                points[c] = Suggestion { black, white, mid: m, ..points[c] };
            }
        }
    }
    Some(points)
}

/// The midtones' colour on their own channels, read on the quarter of them nearest grey through the
/// two-point windows; `None` when even those stand past `NEUTRAL_CHROMA`.
fn neutral_mid(cells: &[[f32; 3]], luma: &[f32], (lo, hi): (f32, f32), points: &[Suggestion; 3]) -> Option<[f32; 3]> {
    let through = |c: usize, x: f32| (x - points[c].black) / (points[c].white - points[c].black);
    let tones = lo + MID_TONES[0] * (hi - lo)..=lo + MID_TONES[1] * (hi - lo);
    let mut mids: Vec<(usize, f32)> = (0..cells.len())
        .filter(|&i| tones.contains(&luma[i]))
        .map(|i| {
            let t = [0, 1, 2].map(|c| through(c, cells[i][c]));
            (i, t.iter().copied().fold(f32::MIN, f32::max) - t.iter().copied().fold(f32::MAX, f32::min))
        })
        .collect();
    if mids.is_empty() {
        return None;
    }
    mids.sort_by(|a, b| a.1.total_cmp(&b.1));
    let near = &mids[..((mids.len() as f32 * MID_KEPT) as usize).max(1)];
    let median = |values: Vec<f32>| {
        let mut v = values;
        v.sort_by(f32::total_cmp);
        v[v.len() / 2]
    };
    if median(near.iter().map(|m| m.1).collect()) >= NEUTRAL_CHROMA {
        return None;
    }
    Some([0, 1, 2].map(|c| median(near.iter().map(|&(i, _)| cells[i][c]).collect())))
}

/// The window and mid laying `x`'s three tones on `y`'s through `((x - black) / (white - black))^gamma`:
/// for a gamma, the dark and the light fix the window; the gamma is bisected until the middle lands.
pub fn three_point(x: [f32; 3], y: [f32; 3]) -> Option<(f32, f32, f32)> {
    let range = oni_settings::levels::Levels::MID_RANGE;
    let window = |gamma: f32| {
        let (ud, ul) = (y[0].powf(1.0 / gamma), y[2].powf(1.0 / gamma));
        let span = (x[2] - x[0]) / (ul - ud);
        let black = x[0] - ud * span;
        (black, black + span)
    };
    let miss = |gamma: f32| {
        let (black, white) = window(gamma);
        ((x[1] - black) / (white - black)).max(0.0).powf(gamma) - y[1]
    };
    // Gamma is ln 0.5 / ln mid: the mid range's ends bound it.
    let (mut a, mut b) = (0.5f32.ln() / range.1.ln(), 0.5f32.ln() / range.0.ln());
    if !(x[2] - x[0] >= MINIMUM_SPAN) || !(y[0] < y[1] && y[1] < y[2]) || miss(a).signum() == miss(b).signum() {
        return None;
    }
    for _ in 0..48 {
        let m = (a + b) / 2.0;
        if miss(m).signum() == miss(a).signum() { a = m } else { b = m }
    }
    let gamma = (a + b) / 2.0;
    let (black, white) = window(gamma);
    Some((black, white, 0.5f32.powf(1.0 / gamma)))
}

/// `AutoLevels.applied`: the balance laid on the frame, its key named when one is.
pub fn applied(b: &Balance, settings: &PipelineSettings, method: Option<&str>) -> PipelineSettings {
    let mut out = balance::applied(&b.core(), settings);
    if let Some(m) = method {
        out.balance_method = Some(m.to_string());
    }
    out
}

/// `AutoLevels.carried`: ends placed under the frame's own inner points, which carry its look.
pub fn carried(ends: &[Suggestion], settings: &PipelineSettings) -> PipelineSettings {
    let mut out = settings.clone();
    for (index, channel) in LevelsChannel::PER_CHANNEL.into_iter().enumerate() {
        let Some(end) = ends.get(index) else { continue };
        let kept = out.levels.get(channel);
        let end = Suggestion { mid: kept.mid, shadows: kept.shadows, highlights: kept.highlights, ..*end };
        if let Some(levels) = out.levels.get_mut(channel) {
            *levels = balance::placed(&end, settings.mode);
        }
    }
    out
}

/// The mean of each handle over several proposals.
pub fn mean(proposals: &[Suggestion]) -> Option<Suggestion> {
    if proposals.is_empty() {
        return None;
    }
    let n = proposals.len() as f32;
    let over = |f: fn(&Suggestion) -> f32| proposals.iter().fold(0.0, |a, p| a + f(p)) / n;
    Some(Suggestion {
        black: over(|p| p.black),
        white: over(|p| p.white),
        mid: over(|p| p.mid),
        shadows: over(|p| p.shadows),
        highlights: over(|p| p.highlights),
    })
}
