//! The dye separation: the stock's measured dyes unmixed on the scan's densities before the windows; off for passes,
//! a positive and black and white.

use super::*;
use crate::film::dyes::{self, Dyes};
use oni_render::DyeSeparation;

/// `Separation.dose`: narrowband passes separated the dyes in the capture, and a positive renders as
/// a plain RAW, its dyes never unmixed (owner's call); a digital photo's negative is unmixed as any is.
pub fn separation_dose(s: &PipelineSettings) -> f32 {
    if s.rgb_passes.is_none() && !s.mode.reads_positive() && !s.inverted_source { s.separation } else { 0.0 }
}

/// The measured dyes a frame unmixes: its stock's, where the separation runs at all.
pub fn stock_dyes(s: &PipelineSettings) -> Option<&'static Dyes> {
    (!s.mode.is_monochrome() && separation_dose(s) > 0.0).then(|| dyes::dyes(s.dye_set))
}

/// The kernel's arguments for the frame's measured dyes: above the bare film its windows open on, at its dose,
/// each dye toned onto the grey its windows print neutral.
pub fn stock_separation(s: &PipelineSettings) -> Option<DyeSeparation> {
    let guard = oni_settings::axis::density_axis::log_guard(rendered_ceiling(s));
    stock_amounts(s).map(|d| toned(d, &s.levels, guard))
}

/// The dye amounts a paper prints: a negative's measured dyes at its dose, a slide's E-6 dyes unmixed whole, as the
/// enlarger passes them; ONI's print reads a slide as a plain RAW.
pub fn paper_amounts(s: &PipelineSettings) -> Option<DyeSeparation> {
    if s.mode.reads_positive() {
        return Some(dyes::slide_dyes().separation(crate::paper::film_ends(s).0, 1.0));
    }
    stock_amounts(s)
}

/// The frame's measured dyes at its dose, their amounts read as they are.
pub fn stock_amounts(s: &PipelineSettings) -> Option<DyeSeparation> {
    let (low, high) = oni_settings::blocks::separation::RANGE;
    stock_dyes(s).map(|d| d.separation(s.levels.windows().map(|w| w.black), s.separation.clamp(low, high)))
}

/// The lightest density a tone is drawn from, ten times a clear film's light: past it a scan holds no picture.
const TONE_LIGHTEST: f32 = -1.0;
const TONE_SAMPLES: usize = 512;

/// `d` toned on `levels`: each neutral grey they print, from the lightest density to the guard's, is unmixed, and
/// each dye's amount there reads that grey's own density, so a grey prints as on the scan's densities, a colour
/// on its dyes. At no dose each tone is the identity.
pub fn toned(d: DyeSeparation, levels: &LevelsSet, log_guard: f32) -> DyeSeparation {
    let w = levels.windows();
    let densest = -log_guard.log10();
    // The levels whose greys reach both ends on every channel, a share under the black read straight on.
    let share = |c: usize, density: f32| (density - w[c].black) / (w[c].white - w[c].black);
    let level_of = |c: usize, s: f32| if s > 0.0 { s.powf(w[c].gamma()) } else { s };
    let ends = |density: f32| (0..3).map(move |c| level_of(c, share(c, density))).filter(|v| v.is_finite());
    let from = ends(TONE_LIGHTEST).chain(ends(densest)).fold(0.0, f32::min);
    let to = ends(TONE_LIGHTEST).chain(ends(densest)).fold(1.0, f32::max);
    let greys: Vec<([f32; 3], [f32; 3])> = (0..TONE_SAMPLES)
        .map(|i| {
            let level = from + (to - from) * i as f32 / (TONE_SAMPLES - 1) as f32;
            // The share of each window that prints `level` neutral, through its own gamma inside it.
            let density: [f32; 3] = std::array::from_fn(|c| {
                let share = if level > 0.0 { level.powf(1.0 / w[c].gamma()) } else { level };
                w[c].black + share * (w[c].white - w[c].black)
            });
            (d.separated(std::array::from_fn(|c| density[c] - d.black[c])), density)
        })
        .collect();
    let mut out = d;
    for c in 0..3 {
        // Each dye along the greys, its amount rising: a step that would not rise is passed over.
        let mut rising: Vec<(f32, f32)> = Vec::with_capacity(greys.len());
        for (amounts, density) in &greys {
            if rising.last().is_none_or(|(a, _)| amounts[c] > *a) {
                rising.push((amounts[c], density[c]));
            }
        }
        // Windows a hand closed or crossed print no grey along a rising dye: each amount then reads as its density.
        if rising.len() < 2 {
            rising = vec![(-1.0, d.black[c] - 1.0), (5.0, d.black[c] + 5.0)];
        }
        let (first, last) = (rising[0].0, rising[rising.len() - 1].0);
        let step = (last - first) / (oni_render::DYE_TONE_NODES - 1) as f32;
        out.tone_low[c] = first;
        out.tone_step[c] = step;
        for (j, node) in out.tone.iter_mut().enumerate() {
            let at = first + step * j as f32;
            let hi = rising.partition_point(|(a, _)| *a <= at).clamp(1, rising.len() - 1);
            let ((a0, d0), (a1, d1)) = (rising[hi - 1], rising[hi]);
            node[c] = d0 + (at - a0) / (a1 - a0) * (d1 - d0);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_positive_read_as_its_own_negative_unmixes_no_dye() {
        let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
        s.separation = 0.8;
        assert_eq!(separation_dose(&s), 0.8);
        s.inverted_source = true;
        // Twin above: the same negative, not inverted, takes its dose.
        assert_eq!(separation_dose(&s), 0.0);
    }

    #[test]
    fn closed_or_crossed_windows_tone_each_dye_as_its_density_and_render_finite() {
        let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
        s.levels.red.white = s.levels.red.black;
        s.levels.green = Levels { black: 1.2, white: 0.3, ..s.levels.green };
        let k = stage47(&s);
        let d = k.dyes.expect("a colour negative's dyes");
        for x in [[0.5, 0.2, 0.1, 1.0], [0.001, 0.3, 0.9, 1.0], [2.0, 2.0, 2.0, 1.0]] {
            assert!(oni_render::cpu::stage47(x, &k).iter().all(|v| v.is_finite()), "{x:?}");
        }
        // Every window closed: no grey rises along any dye, each amount reads as its density.
        let mut closed = s.clone();
        for w in [&mut closed.levels.red, &mut closed.levels.green, &mut closed.levels.blue] {
            w.white = w.black;
        }
        let shut = stage47(&closed).dyes.unwrap();
        assert!((0..3).all(|c| (shut.toned([0.4; 3])[c] - (0.4 + shut.black[c])).abs() < 1e-5), "{:?}", shut.toned([0.4; 3]));
        assert!(d.toned([0.4; 3]).iter().all(|v| v.is_finite()));
        // Twin: open windows tone the amount onto their grey, another density.
        let open = stage47(&PipelineSettings::rest(ConversionMode::Negative, true)).dyes.unwrap();
        assert!((open.toned([0.4; 3])[0] - (0.4 + open.black[0])).abs() > 1e-2);
    }

    #[test]
    fn a_positive_never_unmixes_its_dyes_and_rests_on_the_base_ceiling() {
        let mut s = PipelineSettings::rest(ConversionMode::Positive, true);
        s.separation = 0.6;
        s.density_ceiling = 2.4;
        let k = stage47(&s);
        assert_eq!(separation_dose(&s), 0.0);
        assert!(k.dyes.is_none(), "an old sidecar's dose unmixes nothing");
        assert_eq!(k.pedestal, oni_settings::axis::density_axis::pedestal(oni_settings::axis::density_axis::base()));
        // Twin: a negative keeps its dose and its own ceiling.
        s.mode = ConversionMode::Negative;
        let k = stage47(&s);
        assert_eq!(separation_dose(&s), 0.6);
        assert_eq!(k.dyes.map(|d| d.dose), Some(0.6));
        assert_eq!(k.pedestal, oni_settings::axis::density_axis::pedestal(2.4));
    }
}
