//! The camera matrices a former Mac version folded into every RAW decode, retired: a frame edited
//! under one keeps its look through its gains, until its roll is measured again.

use crate::axis::density_axis;
use crate::blocks::Gains;
use super::chain::MATRICES_RETIRED_VERSION;
use crate::{ConversionMode, PipelineSettings};

pub type M3 = [[f64; 3]; 3];

fn apply(m: &M3, v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|r| (0..3).map(|c| m[r][c] * v[c]).sum())
}

fn inverse(m: &M3) -> Option<M3> {
    let cof = |r: usize, c: usize| {
        let (r0, r1, c0, c1) = ((r + 1) % 3, (r + 2) % 3, (c + 1) % 3, (c + 2) % 3);
        m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
    };
    let det: f64 = (0..3).map(|c| m[0][c] * cof(0, c)).sum();
    (det.abs() > 1e-9).then(|| std::array::from_fn(|r| std::array::from_fn(|c| cof(c, r) / det)))
}

/// A matrix's move on greys in stops, each row's sum.
pub fn on_greys(matrix: &M3) -> Option<[f32; 3]> {
    let sums = matrix.map(|row| row.iter().sum::<f64>());
    sums.iter().all(|s| s.is_finite() && *s > 0.0).then(|| sums.map(|s| libm::log2(s) as f32))
}

/// The stops that keep each channel's black point on the light it stood on, the film's base on a
/// negative, so its points need no move; greys' move when that light is out of reach.
pub fn on_base(settings: &PipelineSettings, matrix: &M3) -> Option<[f32; 3]> {
    let p = f64::from(density_axis::pedestal(settings.density_ceiling));
    let black = [settings.levels.red.black, settings.levels.green.black, settings.levels.blue.black];
    // Stage 47 reads density as -log10((x + pedestal) * gain): the decode's value under each black point.
    let before: [f64; 3] = std::array::from_fn(|c| libm::pow(10.0, -f64::from(black[c])) / libm::exp2(f64::from(settings.gains.stops[c])) - p);
    let after = inverse(matrix).map(|m| apply(&m, before));
    match after {
        Some(after) if (0..3).all(|c| before[c] > 0.0 && after[c] > 0.0) => {
            Some(std::array::from_fn(|c| libm::log2((before[c] + p) / (after[c] + p)) as f32))
        }
        _ => on_greys(matrix),
    }
}

/// A matrix's move on each channel read from a pass of its own, lit by that colour alone: its diagonal.
pub fn on_passes(matrix: &M3) -> Option<[f32; 3]> {
    let d = [matrix[0][0], matrix[1][1], matrix[2][2]];
    d.iter().all(|v| v.is_finite() && *v > 0.0).then(|| d.map(|v| libm::log2(v) as f32))
}

/// A frame the hand touched: a slide, read as itself or as its own negative, decodes plain, and a
/// frame at rest has no look to keep.
fn edited(settings: &PipelineSettings) -> bool {
    settings.mode != ConversionMode::Positive
        && !settings.inverted_source
        && *settings != PipelineSettings::rest(ConversionMode::Negative, settings.decoded_from_mosaic)
}

/// The frame's gains and its roll's moved alike, so a double click lands where the look stood.
pub fn compensated(settings: &PipelineSettings, matrix: &M3) -> PipelineSettings {
    let stops = if settings.rgb_passes.is_some() { on_passes(matrix) } else { on_base(settings, matrix) };
    let Some(stops) = stops else { return settings.clone() };
    let moved = |g: [f32; 3]| std::array::from_fn(|c| (g[c] + stops[c]).clamp(Gains::RANGE.0, Gains::RANGE.1));
    let mut out = settings.clone();
    out.gains.stops = moved(out.gains.stops);
    if let Some(r) = &mut out.roll_reference {
        r.stops = moved(r.stops);
    }
    out
}

/// `compensated` for an edited frame whose sidecar predates the retirement; the camera is asked
/// last, as asking reads the file.
pub fn caught_up(version: i64, settings: PipelineSettings, matrix: &dyn Fn() -> Option<M3>) -> PipelineSettings {
    if version >= MATRICES_RETIRED_VERSION || !edited(&settings) {
        return settings;
    }
    match matrix() {
        Some(matrix) => compensated(&settings, &matrix),
        None => settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::RollReference;

    pub(crate) const Z_F: M3 = [[1.1242, -0.1665, -0.0092], [0.0277, 0.9493, -0.0627], [0.0104, -0.0133, 0.8966]];

    fn edited_negative() -> PipelineSettings {
        let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
        s.print.exposure = 0.4;
        s.gains.stops = [0.1, 0.0, -0.2];
        (s.levels.red.black, s.levels.green.black, s.levels.blue.black) = (0.35, 0.62, 0.9);
        s.roll_reference = Some(RollReference { id: crate::Id::new(), stops: [0.3, 0.0, -0.4], black: [0.0; 3], white: [1.0; 3], base: None, median: None });
        s
    }

    #[test]
    fn a_matrix_moves_greys_by_its_rows_sums_and_an_orange_base_otherwise() {
        let s = on_greys(&Z_F).unwrap();
        let grey = Z_F.map(|row| row.iter().sum::<f64>());
        assert!((0..3).all(|c| (2f64.powf(f64::from(s[c])) - grey[c]).abs() < 1e-6), "{s:?} {grey:?}");
        assert!(on_greys(&[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.5, -0.5]]).is_none());
        // A neutral base moves about as greys do; an orange one takes the matrix's crossed terms too.
        let mut neutral = edited_negative();
        neutral.gains.stops = [0.0; 3];
        (neutral.levels.red.black, neutral.levels.green.black, neutral.levels.blue.black) = (0.5, 0.5, 0.5);
        let n = on_base(&neutral, &Z_F).unwrap();
        assert!((0..3).all(|c| (n[c] - s[c]).abs() < 0.02), "{n:?} {s:?}");
        let orange = on_base(&edited_negative(), &Z_F).unwrap();
        assert!((0..3).any(|c| (orange[c] - s[c]).abs() > 0.05), "{orange:?} {s:?}");
    }

    #[test]
    fn a_frame_of_three_passes_moves_by_the_matrix_diagonal() {
        let mut s = edited_negative();
        s.rgb_passes = Some(crate::frame::RgbPasses { red: "R.NEF".into(), green: "G.NEF".into(), blue: "B.NEF".into(), registration: None });
        let out = compensated(&s, &Z_F);
        let diagonal = [Z_F[0][0], Z_F[1][1], Z_F[2][2]].map(|d| d.log2() as f32);
        assert!((0..3).all(|c| (out.gains.stops[c] - s.gains.stops[c] - diagonal[c]).abs() < 1e-6), "{:?}", out.gains);
        // Twin: the same frame from one capture moves by its base, not the diagonal.
        let one = compensated(&edited_negative(), &Z_F);
        assert!((0..3).any(|c| (one.gains.stops[c] - s.gains.stops[c] - diagonal[c]).abs() > 0.01), "{:?}", one.gains);
    }

    #[test]
    fn an_edited_negative_keeps_its_look_and_a_slide_or_a_frame_at_rest_is_left_alone() {
        let s = edited_negative();
                let by = on_base(&s, &Z_F).unwrap();
        let out = caught_up(MATRICES_RETIRED_VERSION - 1, s.clone(), &|| Some(Z_F));
        let r = out.roll_reference.as_ref().unwrap();
        assert!((0..3).all(|c| (out.gains.stops[c] - s.gains.stops[c] - by[c]).abs() < 1e-6), "{:?}", out.gains);
        assert!((0..3).all(|c| (r.stops[c] - s.roll_reference.as_ref().unwrap().stops[c] - by[c]).abs() < 1e-6), "{r:?}");
        // Twins: a camera without a matrix, a sidecar of this version, a slide read either way and a
        // frame at rest keep their gains, all but the first without the file being asked.
        assert_eq!(caught_up(MATRICES_RETIRED_VERSION - 1, s.clone(), &|| None), s);
        let unasked = || -> Option<M3> { panic!("the camera was asked") };
        assert_eq!(caught_up(MATRICES_RETIRED_VERSION, s.clone(), &unasked), s);
        let mut slide = s.clone();
        slide.mode = ConversionMode::Positive;
        assert_eq!(caught_up(MATRICES_RETIRED_VERSION - 1, slide.clone(), &unasked), slide);
        let mut as_negative = s.clone();
        as_negative.inverted_source = true;
        assert_eq!(caught_up(MATRICES_RETIRED_VERSION - 1, as_negative.clone(), &unasked), as_negative);
        let rest = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_eq!(caught_up(MATRICES_RETIRED_VERSION - 1, rest.clone(), &unasked), rest);
    }
}
