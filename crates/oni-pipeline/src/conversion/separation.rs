//! The dye separation: the family's rows dosed from the identity, the chroma kept, off for passes.

use super::*;

/// `Separation.Family.dyeRows`, output channel per row.
pub fn dye_rows(family: SeparationFamily) -> M3 {
    match family {
        SeparationFamily::KodakC41 => [[1.0000, -0.2379, -0.0092], [-0.0346, 1.0000, -0.0136], [-0.0327, -0.0790, 1.0000]],
        SeparationFamily::FujiC41 => [[1.0000, -0.3034, -0.0059], [-0.0688, 1.0000, -0.0138], [-0.0129, -0.0655, 1.0000]],
        SeparationFamily::GenericC41 => [[1.0000, -0.2706, -0.0076], [-0.0517, 1.0000, -0.0137], [-0.0228, -0.0723, 1.0000]],
        SeparationFamily::Ecn2 => [[1.0000, -0.2706, -0.0082], [-0.0995, 1.0000, -0.0252], [0.0260, -0.1023, 1.0000]],
        SeparationFamily::E6 => [[1.0000, -0.3584, 0.0067], [-0.3109, 1.0000, -0.0588], [-0.0269, -0.2385, 1.0000]],
    }
}

fn mul(m: &M3, v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `Separation.matrix`: the identity at zero, the family's rows normalised to sum to one at one.
pub fn separation_matrix(family: SeparationFamily, strength: f32) -> M3 {
    let s = strength.clamp(oni_settings::blocks::separation::RANGE.0, oni_settings::blocks::separation::RANGE.1);
    let full = dye_rows(family).map(|r| {
        let sum = r[0] + r[1] + r[2];
        r.map(|x| x / sum)
    });
    std::array::from_fn(|i| std::array::from_fn(|j| IDENTITY[i][j] + s * (full[i][j] - IDENTITY[i][j])))
}

/// `Separation.chromaKept`: `m` divided on the plane of colours by its gain there, the square root of
/// its determinant on that plane, so a dose turns hues and keeps their mean saturation.
pub fn chroma_kept(m: M3) -> M3 {
    let u = [1.0 / 2f32.sqrt(), -1.0 / 2f32.sqrt(), 0.0];
    let v = [1.0 / 6f32.sqrt(), 1.0 / 6f32.sqrt(), -2.0 / 6f32.sqrt()];
    let (a, b, c, d) = (dot(u, mul(&m, u)), dot(u, mul(&m, v)), dot(v, mul(&m, u)), dot(v, mul(&m, v)));
    let gain = (a * d - b * c).abs().sqrt();
    if m == IDENTITY || gain <= 1e-6 {
        return m;
    }
    m.map(|r| r.map(|x| x * (1.0 / gain) + (1.0 / 3.0) * (1.0 - 1.0 / gain)))
}

/// `Separation.rows`: the named family, else the mode's resting one; monochrome has nothing to unmix.
pub fn separation_rows(family: Option<SeparationFamily>, strength: f32, mode: ConversionMode) -> M3 {
    match family.or(SeparationFamily::resting(mode)) {
        Some(f) if !mode.is_monochrome() => chroma_kept(separation_matrix(f, strength)),
        _ => IDENTITY,
    }
}

/// `Separation.dose`: narrowband passes separated the dyes in the capture, and a positive renders as
/// a plain RAW, its dyes never unmixed (owner's call).
pub fn separation_dose(s: &PipelineSettings) -> f32 {
    if s.rgb_passes.is_none() && s.mode != ConversionMode::Positive && !s.inverted_source { s.separation } else { 0.0 }
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
    fn the_separation_keeps_greys_and_the_mean_saturation() {
        let rows = separation_rows(Some(SeparationFamily::KodakC41), 0.6, ConversionMode::Negative);
        for r in rows {
            assert!((r[0] + r[1] + r[2] - 1.0).abs() < 1e-6, "a grey is a fixed point: {r:?}");
        }
        assert_eq!(separation_rows(Some(SeparationFamily::KodakC41), 0.0, ConversionMode::Negative), IDENTITY);
        assert_eq!(separation_rows(None, 0.6, ConversionMode::Monochrome), IDENTITY);
        // Twin: the bare dose, without `chromaKept`, multiplies the chroma and differs.
        let bare = separation_matrix(SeparationFamily::KodakC41, 0.6);
        assert!((0..3).any(|i| (0..3).any(|j| (bare[i][j] - rows[i][j]).abs() > 1e-3)));
    }

    #[test]
    fn a_positive_never_unmixes_its_dyes_and_rests_on_the_base_ceiling() {
        let mut s = PipelineSettings::rest(ConversionMode::Positive, true);
        s.separation = 0.6;
        s.separation_family = Some(SeparationFamily::E6);
        s.density_ceiling = 2.4;
        let k = stage47(&s);
        assert_eq!(separation_dose(&s), 0.0);
        assert_eq!([k.sep_r, k.sep_g, k.sep_b], IDENTITY, "an old sidecar's dose unmixes nothing");
        assert_eq!(k.pedestal, oni_settings::axis::density_axis::pedestal(oni_settings::axis::density_axis::base()));
        // Twin: a negative keeps its dose and its own ceiling.
        s.mode = ConversionMode::Negative;
        let k = stage47(&s);
        assert_eq!(separation_dose(&s), 0.6);
        assert_ne!([k.sep_r, k.sep_g, k.sep_b], IDENTITY);
        assert_eq!(k.pedestal, oni_settings::axis::density_axis::pedestal(2.4));
    }
}
