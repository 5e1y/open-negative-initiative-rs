//! `AutoLevels`: where the information in a channel begins and ends, placed in two steps, the gains
//! first, then each channel's ends at those gains, read off a measured histogram.

use crate::histogram::Histogram;
use oni_core::balance::Suggestion;
use oni_settings::axis::graduation;
use oni_settings::blocks::Gains;
use oni_settings::levels::LevelsChannel;
use oni_settings::ConversionMode;

/// The narrowest span a suggestion may propose, a share of the measured window: below it the
/// channel is flat and the honest answer is none.
pub const MINIMUM_SPAN: f32 = 0.02;

/// Green, the channel the other two are brought onto.
pub const REFERENCE: usize = 1;

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

/// Each channel's window at rest on `mode`'s axis, as a balance lays it.
pub fn resting(mode: ConversionMode) -> [Suggestion; 3] {
    let ceiling = oni_settings::axis::density_axis::base();
    LevelsChannel::PER_CHANNEL.map(|c| {
        let w = oni_settings::levels::LevelsSet::resting(c, mode, ceiling);
        Suggestion { black: graduation::fraction(w.black, mode), white: graduation::fraction(w.white, mode), mid: w.mid, shadows: w.shadows, highlights: w.highlights }
    })
}
