//! Every value the panel draws: its label, travel, rest, readout and the store knob it moves, copied
//! from the Swift panes (`ColorPane`, `PrintPanel`, `SettingsPane`, `DetailsPane`, …).

use oni_core::knob::{CHROMA_FORCE_RANGE, SAUCE_RANGE};
use oni_settings::blocks::{ChromaDenoise, Gains, Grain, GrainColour, Halation, PaperCorrection, Sharpen, Texture, Vignette, separation};
use oni_settings::frame::FlatField;
use oni_settings::lens::LensCorrection;
use oni_settings::tone::{Print, color_density};
use oni_settings::white;
use oni_settings::VirtualFilm;
use crate::track::Bend;

mod block;
mod knob;
mod names;
mod readout;

pub use block::Block;

pub use readout::{Axis, Readout, Unit, number};

/// One slider's value, its index the `field` the Slint `Store` passes back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Field {
    Temperature,
    Tint,
    ColorDensity,
    Exposure,
    Highlights,
    Shadows,
    WhitePoint,
    BlackPoint,
    Contrast,
    Vignette,
    Chrome,
    BalanceStrength,
    DensityCeiling,
    DyeSeparation,
    Angle,
    SharpenAmount,
    SharpenRadius,
    SharpenHalo,
    ChromaAmount,
    ChromaRadius,
    Texture,
    PaperRed,
    PaperGreen,
    PaperBlue,
    GainRed,
    GainGreen,
    GainBlue,
    FlatAmount,
    FlatTemperature,
    FlatTint,
    GainTemperature,
    GainTint,
    RedWhitePoint,
    GreenWhitePoint,
    BlueWhitePoint,
    RedBlackPoint,
    GreenBlackPoint,
    BlueBlackPoint,
    WhiteKelvin,
    WhiteTint,
    GrainAmount,
    GrainSize,
    HalationAmount,
    HalationSize,
    Distortion,
    FringeRed,
    FringeBlue,
    LensVignetting,
    HalationSensitivity,
    RedMid,
    GreenMid,
    BlueMid,
    RedContrast,
    GreenContrast,
    BlueContrast,
    GrainColourBlack,
    GrainColourWhite,
    FilmExposure,
    FilmRed,
    FilmGreen,
    FilmBlue,
    FilmRedBlack,
    FilmGreenBlack,
    FilmBlueBlack,
    FilmRedWhite,
    FilmGreenWhite,
    FilmBlueWhite,
}

#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub label: &'static str,
    pub range: (f32, f32),
    /// How the track lays the range under the hand.
    pub bend: Bend,
    pub readout: Readout,
    /// The colour axis its track runs along, whose end the readout names before its number.
    pub axis: Option<Axis>,
    /// The track runs the other way: right lifts the black, the stored value falls.
    pub inverted: bool,
}

impl Field {
    pub const ALL: [Field; 67] = [
        Field::Temperature,
        Field::Tint,
        Field::ColorDensity,
        Field::Exposure,
        Field::Highlights,
        Field::Shadows,
        Field::WhitePoint,
        Field::BlackPoint,
        Field::Contrast,
        Field::Vignette,
        Field::Chrome,
        Field::BalanceStrength,
        Field::DensityCeiling,
        Field::DyeSeparation,
        Field::Angle,
        Field::SharpenAmount,
        Field::SharpenRadius,
        Field::SharpenHalo,
        Field::ChromaAmount,
        Field::ChromaRadius,
        Field::Texture,
        Field::PaperRed,
        Field::PaperGreen,
        Field::PaperBlue,
        Field::GainRed,
        Field::GainGreen,
        Field::GainBlue,
        Field::FlatAmount,
        Field::FlatTemperature,
        Field::FlatTint,
        Field::GainTemperature,
        Field::GainTint,
        Field::RedWhitePoint,
        Field::GreenWhitePoint,
        Field::BlueWhitePoint,
        Field::RedBlackPoint,
        Field::GreenBlackPoint,
        Field::BlueBlackPoint,
        Field::WhiteKelvin,
        Field::WhiteTint,
        Field::GrainAmount,
        Field::GrainSize,
        Field::HalationAmount,
        Field::HalationSize,
        Field::Distortion,
        Field::FringeRed,
        Field::FringeBlue,
        Field::LensVignetting,
        Field::HalationSensitivity,
        Field::RedMid,
        Field::GreenMid,
        Field::BlueMid,
        Field::RedContrast,
        Field::GreenContrast,
        Field::BlueContrast,
        Field::GrainColourBlack,
        Field::GrainColourWhite,
        Field::FilmExposure,
        Field::FilmRed,
        Field::FilmGreen,
        Field::FilmBlue,
        Field::FilmRedBlack,
        Field::FilmGreenBlack,
        Field::FilmBlueBlack,
        Field::FilmRedWhite,
        Field::FilmGreenWhite,
        Field::FilmBlueWhite,
    ];

    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|f| *f == self).unwrap_or(0) as i32
    }

    pub fn from_index(i: i32) -> Option<Field> {
        Self::ALL.get(usize::try_from(i).ok()?).copied()
    }

    pub fn block(self) -> Option<Block> {
        Block::ALL.into_iter().find(|b| b.fields().contains(&self))
    }

    /// Whether the panel shows this value in `mode`: a digital photo's white in place of the print's
    /// filters, so a block's reset and its key read only what it shows.
    pub fn shown_in(self, mode: oni_settings::ConversionMode) -> bool {
        match self {
            Field::WhiteKelvin | Field::WhiteTint => !mode.is_film(),
            Field::GrainAmount | Field::GrainSize | Field::GrainColourBlack | Field::GrainColourWhite => !mode.is_film(),
            Field::HalationAmount | Field::HalationSize | Field::HalationSensitivity => !mode.is_film(),
            Field::Distortion | Field::FringeRed | Field::FringeBlue | Field::LensVignetting => !mode.is_film(),
            f if f.block() == Some(Block::Film) => !mode.is_film(),
            Field::Temperature | Field::Tint => mode.is_film(),
            _ => true,
        }
    }

    /// The panel's spec for this value on a mosaic colour negative, the only mode the shell opens.
    pub fn spec(self) -> Spec {
        let s = |label, range, readout| Spec { label, range, bend: Bend::Even, readout, axis: None, inverted: false };
        let warmth = |spec: Spec| Spec { axis: Some(Axis::Warmth), ..spec };
        let tint = |spec: Spec| Spec { axis: Some(Axis::Tint), ..spec };
        let black = |label| Spec { inverted: true, ..s(label, Print::BLACK_POINT_RANGE, Readout::SignedPercent) };
        match self {
            // The print filters' gap in stops: red against blue, then magenta against green.
            Field::Temperature => warmth(s("Temperature", Print::TEMPERATURE_RANGE, Readout::Stops)),
            Field::Tint => tint(s("Tint", Print::TINT_RANGE, Readout::Stops)),
            Field::ColorDensity => s("Saturation", color_density::RANGE, Readout::SignedPercent),
            Field::Exposure => s("Exposure", Print::EXPOSURE_RANGE, Readout::Stops),
            Field::Highlights => s("Highlights", Print::QUARTER_RANGE, Readout::SignedPercent),
            Field::Shadows => s("Shadows", Print::QUARTER_RANGE, Readout::SignedPercent),
            Field::WhitePoint => s("White point", Print::POINT_RANGE, Readout::SignedPercent),
            Field::BlackPoint => black("Black point"),
            Field::Contrast => s("Contrast", Print::CONTRAST_RANGE, Readout::SignedPercent),
            Field::Vignette => s("Vignette", Vignette::RANGE, Readout::Stops),
            Field::Chrome => s("Chrome", SAUCE_RANGE, Readout::Percent),
            Field::BalanceStrength => s("Auto White Balance Strength", oni_settings::blocks::filter_balance::STRENGTH_RANGE, Readout::Percent),
            Field::DensityCeiling => {
                let range = oni_settings::axis::density_axis::mosaic_range();
                s("Ceiling", (*range.start(), *range.end()), Readout::Ceiling)
            }
            Field::DyeSeparation => s("Dye separation", separation::RANGE, Readout::Percent),
            Field::Angle => s("Angle", oni_settings::frame::Geometry::ANGLE_RANGE, Readout::Degrees),
            Field::SharpenAmount => s("Amount", Sharpen::AMOUNT_RANGE, Readout::Times),
            Field::SharpenRadius => Spec {
                bend: Bend::Centre(Sharpen::RADIUS_ANCHOR),
                ..s("Radius", Sharpen::radius_range(), Readout::Pixels)
            },
            Field::SharpenHalo => s("Halo suppression", Sharpen::HALO_RANGE, Readout::Percent),
            Field::ChromaAmount => s("Amount", CHROMA_FORCE_RANGE, Readout::Percent),
            Field::ChromaRadius => s("Radius", ChromaDenoise::RADIUS_RANGE, Readout::Pixels),
            Field::Texture => s("Texture", Texture::RANGE, Readout::SignedTimes),
            // `GainPanel` titled by its channel alone, as the Paper Correction block names them.
            Field::PaperRed => s("Red", PaperCorrection::RANGE, Readout::Stops),
            Field::PaperGreen => s("Green", PaperCorrection::RANGE, Readout::Stops),
            Field::PaperBlue => s("Blue", PaperCorrection::RANGE, Readout::Stops),
            // `GainPanel`'s rows, named by the ends the mode gives them, which the panel lays.
            Field::GainRed => s("Red", Gains::RANGE, Readout::Stops),
            Field::GainGreen => s("Green", Gains::RANGE, Readout::Stops),
            Field::GainBlue => s("Blue", Gains::RANGE, Readout::Stops),
            Field::GainTemperature => s("Temperature", Gains::RANGE, Readout::Stops),
            Field::GainTint => s("Tint", Gains::RANGE, Readout::Stops),
            Field::FlatAmount => s("Amount", FlatField::AMOUNT_RANGE, Readout::Percent),
            Field::FlatTemperature => warmth(s("Temperature", FlatField::COLOUR_RANGE, Readout::SignedPercent)),
            Field::FlatTint => tint(s("Tint", FlatField::COLOUR_RANGE, Readout::SignedPercent)),
            // Each channel's own points, read as Lighting's.
            Field::RedWhitePoint => s("Red white point", Print::POINT_RANGE, Readout::SignedPercent),
            Field::GreenWhitePoint => s("Green white point", Print::POINT_RANGE, Readout::SignedPercent),
            Field::BlueWhitePoint => s("Blue white point", Print::POINT_RANGE, Readout::SignedPercent),
            Field::RedBlackPoint => black("Red black point"),
            Field::GreenBlackPoint => black("Green black point"),
            Field::BlueBlackPoint => black("Blue black point"),
            // Each channel's own mids and contrast, read as Lighting's.
            Field::RedMid => s("Red mid", Print::ZONE_RANGE, Readout::SignedPercent),
            Field::GreenMid => s("Green mid", Print::ZONE_RANGE, Readout::SignedPercent),
            Field::BlueMid => s("Blue mid", Print::ZONE_RANGE, Readout::SignedPercent),
            Field::RedContrast => s("Red contrast", Print::CONTRAST_RANGE, Readout::SignedPercent),
            Field::GreenContrast => s("Green contrast", Print::CONTRAST_RANGE, Readout::SignedPercent),
            Field::BlueContrast => s("Blue contrast", Print::CONTRAST_RANGE, Readout::SignedPercent),
            // A digital photo's white, in kelvin even in mired and in DNG's tint.
            Field::WhiteKelvin => Spec { bend: Bend::Mired, ..s("Temperature", white::KELVIN_RANGE, Readout::Kelvin) },
            Field::WhiteTint => tint(s("Tint", white::TINT_RANGE, Readout::SignedWhole)),
            Field::GrainAmount => s("Grain", Grain::AMOUNT_RANGE, Readout::Percent),
            Field::GrainSize => s("Grain size", Grain::SIZE_RANGE, Readout::Percent),
            // The grain's colour line, its saturation at the picture's black and at its white.
            Field::GrainColourBlack => s("Grain colour at the black point", GrainColour::RANGE, Readout::Percent),
            Field::GrainColourWhite => s("Grain colour at the white point", GrainColour::RANGE, Readout::Percent),
            Field::HalationAmount => s("Halation", Halation::AMOUNT_RANGE, Readout::Percent),
            Field::HalationSize => s("Halation size", Halation::SIZE_RANGE, Readout::Percent),
            // Stops the threshold is lowered by: right, the dimmer light glows too.
            Field::HalationSensitivity => s("Halation sensitivity", Halation::SENSITIVITY_RANGE, Readout::Stops),
            // A lens's corrections, each a share of its slider's reach.
            Field::Distortion => s("Distortion", LensCorrection::RANGE, Readout::SignedWhole),
            Field::FringeRed => s("Red fringe", LensCorrection::RANGE, Readout::SignedWhole),
            Field::FringeBlue => s("Blue fringe", LensCorrection::RANGE, Readout::SignedWhole),
            Field::LensVignetting => s("Vignetting", LensCorrection::RANGE, Readout::SignedWhole),
            // A virtual film's exposure, its negative's gains, and each channel's ends as shares of the window.
            Field::FilmExposure => s("Film exposure", VirtualFilm::EXPOSURE_RANGE, Readout::Stops),
            Field::FilmRed => s("Red ↔ Cyan", Gains::RANGE, Readout::Stops),
            Field::FilmGreen => s("Green ↔ Magenta", Gains::RANGE, Readout::Stops),
            Field::FilmBlue => s("Blue ↔ Yellow", Gains::RANGE, Readout::Stops),
            Field::FilmRedBlack => s("Red black", VirtualFilm::SHARE_RANGE, Readout::Percent),
            Field::FilmGreenBlack => s("Green black", VirtualFilm::SHARE_RANGE, Readout::Percent),
            Field::FilmBlueBlack => s("Blue black", VirtualFilm::SHARE_RANGE, Readout::Percent),
            Field::FilmRedWhite => s("Red white", VirtualFilm::SHARE_RANGE, Readout::Percent),
            Field::FilmGreenWhite => s("Green white", VirtualFilm::SHARE_RANGE, Readout::Percent),
            Field::FilmBlueWhite => s("Blue white", VirtualFilm::SHARE_RANGE, Readout::Percent),
        }
    }

    /// The value's one rest on this frame: where its track's tick stands, where a double click lands
    /// and what its block's reset writes, the frame's import state in its own mode and decode; a white
    /// rests on the frame's As Shot.
    pub fn base(self, settings: &oni_settings::PipelineSettings) -> f32 {
        let mut rest = oni_settings::PipelineSettings::rest(settings.mode, settings.decoded_from_mosaic);
        rest.white = settings.shot;
        if self.block() == Some(Block::SourceBalance) {
            // `restingGains`: a measured roll rests on its window's stops; a slide, on its own measure's.
            rest = settings.clone();
            rest.gains.stops = settings.roll_reference.as_ref().map(|r| r.stops).or(settings.measured_gains).unwrap_or([0.0; 3]);
        }
        if self.block() == Some(Block::Film) {
            // A virtual film rests on its stock's own reference, at the film's speed.
            rest = settings.clone();
            rest.virtual_film = settings.virtual_film.as_ref().map(|f| oni_pipeline::film::stock(f.stock).reference.clone());
        }
        self.knob().get(&rest)
    }

    /// What the track shows for a stored value: flipped where the track runs the other way.
    pub fn shown(self, stored: f32) -> f32 {
        if self.spec().inverted { -stored } else { stored }
    }

    /// The readout of a stored value, in the sense the slider shows it, after the end of its colour
    /// axis it leans to.
    pub fn readout(self, stored: f32) -> String {
        let spec = self.spec();
        spec.readout.along(spec.axis, self.shown(stored))
    }

    /// The readout of stored values from `low` to `high` in the sense the slider shows them, as the
    /// frames shown together span them.
    pub fn span_readout(self, low: f32, high: f32) -> String {
        let spec = self.spec();
        let (a, b) = (self.shown(low), self.shown(high));
        spec.readout.span(spec.axis, a.min(b), a.max(b))
    }

    /// The range the track spans, in the shown sense.
    pub fn track(self) -> (f32, f32) {
        let spec = self.spec();
        if spec.inverted { (-spec.range.1, -spec.range.0) } else { spec.range }
    }
}

#[cfg(test)]
mod tests;
