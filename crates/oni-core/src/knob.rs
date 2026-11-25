//! The scalar controls of the panes, each one value a slider or a key moves: what `Set` and
//! `Step` address, each clamped to its slider's range and reporting the field it writes.

use crate::fields::Field;
use oni_settings::axis::density_axis;
use oni_settings::blocks::{Bloom, ChromaDenoise, Gains, Grain, Halation, PaperCorrection, Sharpen, Texture, Vignette};
use oni_settings::frame::{FlatField, Geometry};
use oni_settings::lens::LensCorrection;
use oni_settings::levels::Levels;
use oni_settings::settings::roll_settings;
use oni_settings::tone::{Print, color_density};
use oni_settings::{PipelineSettings, VirtualFilm};

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

impl Handle {
    pub const ALL: [Handle; 5] = [Handle::Black, Handle::Shadows, Handle::Mid, Handle::Highlights, Handle::White];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Knob {
    Gain(Channel),
    /// The gains' red against blue, half their gap, and green against their mean, the gains' own mean
    /// held: a slide's cast in two moves (owner's call).
    GainTemperature,
    GainTint,
    DensityCeiling,
    /// Balance strength re-doses the filters a key wrote, in one write.
    BalanceStrength,
    Level(Channel, Handle),
    Exposure,
    /// A paper's print lit as Exposure lights it, its blacks and first shadows held.
    Brightness,
    Contrast,
    Shadows,
    Highlights,
    BlackPoint,
    WhitePoint,
    /// A channel's own white and black points, laid on Lighting's.
    ChannelWhite(Channel),
    ChannelBlack(Channel),
    /// A channel's own mids and contrast, after Lighting's curve.
    ChannelMid(Channel),
    ChannelContrast(Channel),
    Temperature,
    Tint,
    /// Temperature and Tint read as an enlarger's C, M and Y in CC, the least at none, a channel's own band.
    Filtration(Channel),
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
    GrainAmount,
    GrainSize,
    HalationAmount,
    HalationSize,
    /// A film's own blur, its deviation on the film.
    Bloom,
    /// A digital photo's lens corrections, each a share of its slider.
    LensDistortion,
    LensRed,
    LensBlue,
    LensVignetting,
    /// The stops of light a digital photo's virtual film takes over its scene's own.
    FilmExposure,
}

impl Knob {
    /// The field a write of this knob moves.
    pub fn field(self) -> Field {
        match self {
            Knob::Gain(_) | Knob::GainTemperature | Knob::GainTint => Field::Gains,
            Knob::DensityCeiling => Field::DensityCeiling,
            Knob::BalanceStrength => Field::BalanceStrength,
            Knob::Level(..) => Field::Levels,
            Knob::Exposure
            | Knob::Brightness
            | Knob::Contrast
            | Knob::Shadows
            | Knob::Highlights
            | Knob::BlackPoint
            | Knob::WhitePoint
            | Knob::ChannelWhite(_)
            | Knob::ChannelBlack(_)
            | Knob::ChannelMid(_)
            | Knob::ChannelContrast(_)
            | Knob::Temperature
            | Knob::Tint
            | Knob::Filtration(_) => Field::Print,
            Knob::Density => Field::Density,
            Knob::Vignette => Field::Vignette,
            Knob::Texture => Field::Texture,
            Knob::SharpenAmount | Knob::SharpenRadius | Knob::SharpenHalo => Field::Sharpen,
            Knob::ChromaForce | Knob::ChromaRadius => Field::Chroma,
            Knob::Chrome => Field::SecretSauce,
            Knob::Paper(_) => Field::PaperCorrection,
            Knob::FlatFieldAmount | Knob::FlatFieldTemperature | Knob::FlatFieldTint => Field::FlatField,
            Knob::Angle => Field::Geometry,
            Knob::GrainAmount | Knob::GrainSize => Field::Grain,
            Knob::HalationAmount | Knob::HalationSize => Field::Halation,
            Knob::Bloom => Field::Bloom,
            Knob::LensDistortion | Knob::LensRed | Knob::LensBlue | Knob::LensVignetting => Field::LensCorrection,
            Knob::FilmExposure => Field::VirtualFilm,
        }
    }

    /// The slider's range on these settings; the levels' handles keep their own order instead.
    pub fn range(self, s: &PipelineSettings) -> Option<(f32, f32)> {
        let mono = s.is_monochrome();
        Some(match self {
            Knob::Gain(_) | Knob::GainTemperature | Knob::GainTint => Gains::RANGE,
            Knob::DensityCeiling => {
                let r = density_axis::range_for(s.decoded_from_mosaic, s.mode);
                (*r.start(), *r.end())
            }
            Knob::BalanceStrength => oni_settings::blocks::filter_balance::STRENGTH_RANGE,
            Knob::Level(_, Handle::Mid) => Levels::MID_RANGE,
            Knob::Level(_, Handle::Shadows) => Levels::SHADOW_RANGE,
            Knob::Level(_, Handle::Highlights) => Levels::HIGHLIGHT_RANGE,
            Knob::Level(..) => return None,
            Knob::Exposure => Print::EXPOSURE_RANGE,
            Knob::Brightness => Print::BRIGHTNESS_RANGE,
            Knob::Contrast if mono => Print::MONOCHROME_CONTRAST_RANGE,
            Knob::Contrast => Print::CONTRAST_RANGE,
            Knob::Shadows | Knob::Highlights => Print::QUARTER_RANGE,
            Knob::BlackPoint | Knob::WhitePoint if mono => Print::MONOCHROME_POINT_RANGE,
            Knob::BlackPoint => Print::BLACK_POINT_RANGE,
            Knob::WhitePoint | Knob::ChannelWhite(_) => Print::POINT_RANGE,
            Knob::ChannelBlack(_) => Print::BLACK_POINT_RANGE,
            Knob::ChannelMid(_) => Print::ZONE_RANGE,
            Knob::ChannelContrast(_) => Print::CONTRAST_RANGE,
            Knob::Temperature => Print::TEMPERATURE_RANGE,
            Knob::Tint => Print::TINT_RANGE,
            Knob::Filtration(_) => Print::FILTER_RANGE,
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
            Knob::GrainAmount => Grain::AMOUNT_RANGE,
            Knob::GrainSize => Grain::SIZE_RANGE,
            Knob::HalationAmount => Halation::AMOUNT_RANGE,
            Knob::HalationSize => Halation::SIZE_RANGE,
            Knob::Bloom => Bloom::RANGE,
            Knob::LensDistortion | Knob::LensRed | Knob::LensBlue | Knob::LensVignetting => LensCorrection::RANGE,
            Knob::FilmExposure => VirtualFilm::EXPOSURE_RANGE,
        })
    }

    pub fn get(self, s: &PipelineSettings) -> f32 {
        let p = &s.print;
        match self {
            Knob::Gain(c) => s.gains.stops[c.index()],
            Knob::GainTemperature => gain_temperature(s.gains.stops),
            Knob::GainTint => gain_tint(s.gains.stops),
            Knob::DensityCeiling => s.density_ceiling,
            Knob::BalanceStrength => s.balance_strength,
            Knob::Level(c, h) => handle(levels(s, c), h),
            Knob::Exposure => p.exposure,
            Knob::Brightness => p.brightness,
            Knob::Contrast => p.contrast,
            Knob::Shadows => p.shadows,
            Knob::Highlights => p.highlights,
            Knob::BlackPoint => p.black_point,
            Knob::WhitePoint => p.white_point,
            Knob::ChannelWhite(c) => p.channel_white[c.index()],
            Knob::ChannelBlack(c) => p.channel_black[c.index()],
            Knob::ChannelMid(c) => p.channel_mid[c.index()],
            Knob::ChannelContrast(c) => p.channel_contrast[c.index()],
            Knob::Temperature => p.red.exposure - p.blue.exposure,
            Knob::Tint => (p.red.exposure + p.blue.exposure) / 2.0 - p.green.exposure,
            Knob::Filtration(c) => p.filters()[c.index()],
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
            Knob::GrainAmount => s.grain.amount,
            Knob::GrainSize => s.grain.size,
            Knob::HalationAmount => s.halation.amount,
            Knob::HalationSize => s.halation.size,
            Knob::Bloom => s.bloom.size,
            Knob::LensDistortion => s.lens_correction.distortion,
            Knob::LensRed => s.lens_correction.red,
            Knob::LensBlue => s.lens_correction.blue,
            Knob::LensVignetting => s.lens_correction.vignetting,
            Knob::FilmExposure => s.film().map_or(0.0, |f| f.exposure),
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
            Knob::GainTemperature => shift_gains(&mut out.gains, GAIN_TEMPERATURE_AXIS, v - Knob::GainTemperature.get(s)),
            Knob::GainTint => shift_gains(&mut out.gains, GAIN_TINT_AXIS, v - Knob::GainTint.get(s)),
            Knob::DensityCeiling => out.density_ceiling = v,
            Knob::BalanceStrength => {}
            Knob::Level(c, h) => set_handle(levels_mut(&mut out, c), h, v),
            Knob::Exposure => out.print.exposure = v,
            Knob::Brightness => out.print.brightness = v,
            Knob::Contrast => out.print.contrast = v,
            Knob::Shadows => out.print.shadows = v,
            Knob::Highlights => out.print.highlights = v,
            Knob::BlackPoint => out.print.black_point = v,
            Knob::WhitePoint => out.print.white_point = v,
            Knob::ChannelWhite(c) => out.print.channel_white[c.index()] = v,
            Knob::ChannelBlack(c) => out.print.channel_black[c.index()] = v,
            Knob::ChannelMid(c) => out.print.channel_mid[c.index()] = v,
            Knob::ChannelContrast(c) => out.print.channel_contrast[c.index()] = v,
            // Each moves the filters along its own axis, so the other reads what it read.
            Knob::Temperature => {
                let by = v - Knob::Temperature.get(s);
                shift_filters(&mut out.print, Print::TEMPERATURE_AXIS, by);
            }
            Knob::Tint => {
                let by = v - Knob::Tint.get(s);
                shift_filters(&mut out.print, Print::TINT_AXIS, by);
            }
            // One band's filter alone, in stops of its light: the three moved together move the photograph's light.
            Knob::Filtration(c) => {
                let mut f = out.print.filters();
                f[c.index()] = v;
                out.print.set_filters(f);
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
            Knob::GrainAmount => out.grain.amount = v,
            Knob::GrainSize => out.grain.size = v,
            Knob::HalationAmount => out.halation.amount = v,
            Knob::HalationSize => out.halation.size = v,
            Knob::Bloom => out.bloom.size = v,
            Knob::LensDistortion => out.lens_correction.distortion = v,
            Knob::LensRed => out.lens_correction.red = v,
            Knob::LensBlue => out.lens_correction.blue = v,
            Knob::LensVignetting => out.lens_correction.vignetting = v,
            Knob::FilmExposure => {
                if let Some(f) = s.film() {
                    out.virtual_film = Some(VirtualFilm { exposure: v, ..f });
                }
            }
        }
        out
    }
}

fn shift_filters(p: &mut Print, axis: [f32; 3], by: f32) {
    let f = p.filters();
    p.set_filters([f[0] + by * axis[0], f[1] + by * axis[1], f[2] + by * axis[2]]);
}

fn handle(l: &Levels, h: Handle) -> f32 {
    match h {
        Handle::Black => l.black,
        Handle::Shadows => l.shadows,
        Handle::Mid => l.mid,
        Handle::Highlights => l.highlights,
        Handle::White => l.white,
    }
}

/// The ends through their setters, which keep the black under the white.
fn set_handle(l: &mut Levels, h: Handle, v: f32) {
    match h {
        Handle::Black => l.set_black(v),
        Handle::Shadows => l.shadows = v,
        Handle::Mid => l.mid = v,
        Handle::Highlights => l.highlights = v,
        Handle::White => l.set_white(v),
    }
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

mod gains;

pub use gains::{gain_temperature, gain_tint};
use gains::{GAIN_TEMPERATURE_AXIS, GAIN_TINT_AXIS, shift_gains};

#[cfg(test)]
mod tests;
