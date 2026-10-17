//! The scalar controls of the panes, each one value a slider or a key moves: what `Set` and
//! `Step` address, each clamped to its slider's range and reporting the field it writes.

use crate::fields::Field;
use oni_settings::axis::density_axis;
use oni_settings::blocks::{ChromaDenoise, Gains, PaperCorrection, Sharpen, Texture, Vignette, separation};
use oni_settings::frame::{FlatField, Geometry};
use oni_settings::levels::Levels;
use oni_settings::settings::roll_settings;
use oni_settings::tone::{Print, color_density};
use oni_settings::PipelineSettings;

/// Chroma denoise's Amount slider.
pub const CHROMA_FORCE_RANGE: (f32, f32) = (0.0, 1.0);
/// Chrome's track, `SaturationCurve.sauceRange`.
pub const SAUCE_RANGE: (f32, f32) = (0.0, 1.5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Red,
    Green,
    Blue,
}

impl Channel {
    pub const ALL: [Channel; 3] = [Channel::Red, Channel::Green, Channel::Blue];

    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Handle {
    Black,
    Shadows,
    Mid,
    Highlights,
    White,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Knob {
    Gain(Channel),
    DensityCeiling,
    /// The dye separation's dose, which a hand dialling it owns from then on.
    Separation,
    /// Balance strength re-doses the filters a key wrote, in one write.
    BalanceStrength,
    Level(Channel, Handle),
    Exposure,
    Contrast,
    Shadows,
    Highlights,
    BlackPoint,
    WhitePoint,
    Temperature,
    Tint,
    Density,
    Vignette,
    Texture,
    SharpenAmount,
    SharpenRadius,
    SharpenHalo,
    ChromaForce,
    ChromaRadius,
    Chrome,
    Paper(Channel),
    FlatFieldAmount,
    FlatFieldTemperature,
    FlatFieldTint,
    Angle,
}

impl Knob {
    /// The field a write of this knob moves.
    pub fn field(self) -> Field {
        match self {
            Knob::Gain(_) => Field::Gains,
            Knob::DensityCeiling => Field::DensityCeiling,
            Knob::Separation => Field::Separation,
            Knob::BalanceStrength => Field::BalanceStrength,
            Knob::Level(..) => Field::Levels,
            Knob::Exposure
            | Knob::Contrast
            | Knob::Shadows
            | Knob::Highlights
            | Knob::BlackPoint
            | Knob::WhitePoint
            | Knob::Temperature
            | Knob::Tint => Field::Print,
            Knob::Density => Field::Density,
            Knob::Vignette => Field::Vignette,
            Knob::Texture => Field::Texture,
            Knob::SharpenAmount | Knob::SharpenRadius | Knob::SharpenHalo => Field::Sharpen,
            Knob::ChromaForce | Knob::ChromaRadius => Field::Chroma,
            Knob::Chrome => Field::SecretSauce,
            Knob::Paper(_) => Field::PaperCorrection,
            Knob::FlatFieldAmount | Knob::FlatFieldTemperature | Knob::FlatFieldTint => Field::FlatField,
            Knob::Angle => Field::Geometry,
        }
    }

    /// The slider's range on these settings; the levels' handles keep their own order instead.
    pub fn range(self, s: &PipelineSettings) -> Option<(f32, f32)> {
        let mono = s.mode.is_monochrome();
        Some(match self {
            Knob::Gain(_) => Gains::RANGE,
            Knob::DensityCeiling => {
                let r = density_axis::range_for(s.decoded_from_mosaic);
                (*r.start(), *r.end())
            }
            Knob::Separation => separation::RANGE,
            Knob::BalanceStrength => oni_settings::blocks::filter_balance::STRENGTH_RANGE,
            Knob::Level(_, Handle::Mid) => Levels::MID_RANGE,
            Knob::Level(_, Handle::Shadows) => Levels::SHADOW_RANGE,
            Knob::Level(_, Handle::Highlights) => Levels::HIGHLIGHT_RANGE,
            Knob::Level(..) => return None,
            Knob::Exposure => Print::EXPOSURE_RANGE,
            Knob::Contrast if mono => Print::MONOCHROME_CONTRAST_RANGE,
            Knob::Contrast => Print::CONTRAST_RANGE,
            Knob::Shadows | Knob::Highlights => Print::QUARTER_RANGE,
            Knob::BlackPoint | Knob::WhitePoint if mono => Print::MONOCHROME_POINT_RANGE,
            Knob::BlackPoint => Print::BLACK_POINT_RANGE,
            Knob::WhitePoint => Print::POINT_RANGE,
            Knob::Temperature => Print::TEMPERATURE_RANGE,
            Knob::Tint => Print::TINT_RANGE,
            Knob::Density => color_density::RANGE,
            Knob::Vignette => Vignette::RANGE,
            Knob::Texture => Texture::RANGE,
            Knob::SharpenAmount => Sharpen::AMOUNT_RANGE,
            Knob::SharpenRadius => Sharpen::radius_range(),
            Knob::SharpenHalo => Sharpen::HALO_RANGE,
            Knob::ChromaForce => CHROMA_FORCE_RANGE,
            Knob::ChromaRadius => ChromaDenoise::RADIUS_RANGE,
            Knob::Chrome => SAUCE_RANGE,
            Knob::Paper(_) => PaperCorrection::RANGE,
            Knob::FlatFieldAmount => FlatField::AMOUNT_RANGE,
            Knob::FlatFieldTemperature | Knob::FlatFieldTint => FlatField::COLOUR_RANGE,
            Knob::Angle => Geometry::ANGLE_RANGE,
        })
    }

    pub fn get(self, s: &PipelineSettings) -> f32 {
        let p = &s.print;
        match self {
            Knob::Gain(c) => s.gains.stops[c.index()],
            Knob::DensityCeiling => s.density_ceiling,
            Knob::Separation => s.separation,
            Knob::BalanceStrength => s.balance_strength,
            Knob::Level(c, h) => {
                let l = levels(s, c);
                match h {
                    Handle::Black => l.black,
                    Handle::Shadows => l.shadows,
                    Handle::Mid => l.mid,
                    Handle::Highlights => l.highlights,
                    Handle::White => l.white,
                }
            }
            Knob::Exposure => p.exposure,
            Knob::Contrast => p.contrast,
            Knob::Shadows => p.shadows,
            Knob::Highlights => p.highlights,
            Knob::BlackPoint => p.black_point,
            Knob::WhitePoint => p.white_point,
            Knob::Temperature => p.red.exposure - p.blue.exposure,
            Knob::Tint => (p.red.exposure + p.blue.exposure) / 2.0 - p.green.exposure,
            Knob::Density => s.density,
            Knob::Vignette => s.vignette.stops,
            Knob::Texture => s.texture.amount,
            Knob::SharpenAmount => s.sharpen.amount,
            Knob::SharpenRadius => s.sharpen.radius,
            Knob::SharpenHalo => s.sharpen.halo,
            Knob::ChromaForce => s.chroma.force,
            Knob::ChromaRadius => s.chroma.radius,
            Knob::Chrome => s.secret_sauce,
            Knob::Paper(c) => s.paper_correction.stops[c.index()],
            Knob::FlatFieldAmount => s.flat_field.amount,
            Knob::FlatFieldTemperature => s.flat_field.temperature,
            Knob::FlatFieldTint => s.flat_field.tint,
            Knob::Angle => s.geometry.angle,
        }
    }

    /// The settings with this knob at `value`, clamped to its range, in one write.
    pub fn set(self, s: &PipelineSettings, value: f32) -> PipelineSettings {
        if !value.is_finite() {
            return s.clone();
        }
        let v = self.range(s).map_or(value, |(lo, hi)| value.clamp(lo, hi));
        if self == Knob::BalanceStrength {
            return roll_settings::dosed(s, v);
        }
        let mut out = s.clone();
        match self {
            Knob::Gain(c) => out.gains.stops[c.index()] = v,
            Knob::DensityCeiling => out.density_ceiling = v,
            Knob::Separation => {
                out.separation = v;
                out.separation_by_hand = true;
            }
            Knob::BalanceStrength => {}
            Knob::Level(c, h) => {
                let l = levels_mut(&mut out, c);
                match h {
                    Handle::Black => l.set_black(v),
                    Handle::Shadows => l.shadows = v,
                    Handle::Mid => l.mid = v,
                    Handle::Highlights => l.highlights = v,
                    Handle::White => l.set_white(v),
                }
            }
            Knob::Exposure => out.print.exposure = v,
            Knob::Contrast => out.print.contrast = v,
            Knob::Shadows => out.print.shadows = v,
            Knob::Highlights => out.print.highlights = v,
            Knob::BlackPoint => out.print.black_point = v,
            Knob::WhitePoint => out.print.white_point = v,
            // Each moves the filters along its own axis, so the other reads what it read.
            Knob::Temperature => {
                let by = v - Knob::Temperature.get(s);
                shift_filters(&mut out.print, Print::TEMPERATURE_AXIS, by);
            }
            Knob::Tint => {
                let by = v - Knob::Tint.get(s);
                shift_filters(&mut out.print, Print::TINT_AXIS, by);
            }
            Knob::Density => out.density = v,
            Knob::Vignette => out.vignette.stops = v,
            Knob::Texture => out.texture.amount = v,
            Knob::SharpenAmount => out.sharpen.amount = v,
            Knob::SharpenRadius => out.sharpen.radius = v,
            Knob::SharpenHalo => out.sharpen.halo = v,
            Knob::ChromaForce => out.chroma.force = v,
            Knob::ChromaRadius => out.chroma.radius = v,
            Knob::Chrome => out.secret_sauce = v,
            Knob::Paper(c) => out.paper_correction.stops[c.index()] = v,
            Knob::FlatFieldAmount => out.flat_field.amount = v,
            Knob::FlatFieldTemperature => out.flat_field.temperature = v,
            Knob::FlatFieldTint => out.flat_field.tint = v,
            Knob::Angle => out.geometry.angle = v,
        }
        out
    }
}

fn shift_filters(p: &mut Print, axis: [f32; 3], by: f32) {
    let f = p.filters();
    p.set_filters([f[0] + by * axis[0], f[1] + by * axis[1], f[2] + by * axis[2]]);
}

fn levels(s: &PipelineSettings, c: Channel) -> &Levels {
    match c {
        Channel::Red => &s.levels.red,
        Channel::Green => &s.levels.green,
        Channel::Blue => &s.levels.blue,
    }
}

fn levels_mut(s: &mut PipelineSettings, c: Channel) -> &mut Levels {
    match c {
        Channel::Red => &mut s.levels.red,
        Channel::Green => &mut s.levels.green,
        Channel::Blue => &mut s.levels.blue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::FieldSet;

    #[test]
    fn a_knob_writes_its_own_field_and_holds_its_range() {
        let s = PipelineSettings::default();
        for knob in [Knob::Exposure, Knob::Gain(Channel::Blue), Knob::SharpenAmount, Knob::Angle, Knob::Chrome] {
            let moved = knob.set(&s, knob.get(&s) + 0.25);
            let fields: Vec<Field> = FieldSet::between(&s, &moved).iter().collect();
            assert_eq!(fields, vec![knob.field()], "{knob:?}");
        }
        assert_eq!(Knob::Exposure.get(&Knob::Exposure.set(&s, 99.0)), Print::EXPOSURE_RANGE.1);
        // Twin: a value inside the range is written as given.
        assert_eq!(Knob::Exposure.get(&Knob::Exposure.set(&s, 0.75)), 0.75);
    }

    #[test]
    fn temperature_and_tint_move_on_axes_that_leave_each_other_alone() {
        let s = PipelineSettings::default();
        let warm = Knob::Temperature.set(&s, 0.8);
        assert!((Knob::Temperature.get(&warm) - 0.8).abs() < 1e-6);
        assert!(Knob::Tint.get(&warm).abs() < 1e-6);
        let both = Knob::Tint.set(&warm, -0.3);
        assert!((Knob::Temperature.get(&both) - 0.8).abs() < 1e-6 && (Knob::Tint.get(&both) + 0.3).abs() < 1e-6);
        // Twin: writing red alone moves both readings, which the axes exist to avoid.
        let mut raw = s.clone();
        raw.print.red.exposure = 0.8;
        assert!(Knob::Tint.get(&raw).abs() > 0.1);
    }

    #[test]
    fn a_dose_by_hand_and_a_strength_redose_as_the_panes_do() {
        let mut s = PipelineSettings::default();
        let dosed = Knob::Separation.set(&s, 1.2);
        assert!(dosed.separation_by_hand);
        s.balance_filters = Some([0.4, 0.0, -0.4]);
        let half = Knob::BalanceStrength.set(&s, 0.5);
        assert_eq!(half.print.red.exposure, 0.2);
        // Twin: a frame no key balanced keeps its filters under the same strength.
        let unbalanced = Knob::BalanceStrength.set(&PipelineSettings::default(), 0.5);
        assert_eq!(unbalanced.print.red.exposure, 0.0);
    }
}
