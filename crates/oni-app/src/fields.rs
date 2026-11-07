//! Every value the panel draws: its label, travel, rest, readout and the store knob it moves, copied
//! from the Swift panes (`ColorPane`, `PrintPanel`, `SettingsPane`, `DetailsPane`, …).

use oni_core::knob::{CHROMA_FORCE_RANGE, Channel, Knob, SAUCE_RANGE};
use oni_settings::blocks::{ChromaDenoise, Gains, PaperCorrection, Sharpen, Texture, Vignette, separation};
use oni_settings::frame::FlatField;
use oni_settings::tone::{Print, color_density};

mod readout;

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
}

/// The blocks a reset key clears, in the order of the Slint `Store`'s `*-modified` flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Block {
    Color,
    Lighting,
    Effects,
    DensityCeiling,
    DyeSeparation,
    Sharpening,
    ChromaDenoise,
    Texture,
    PaperCorrection,
    SourceBalance,
    FlatField,
}

impl Block {
    pub const ALL: [Block; 11] = [
        Block::Color,
        Block::Lighting,
        Block::Effects,
        Block::DensityCeiling,
        Block::DyeSeparation,
        Block::Sharpening,
        Block::ChromaDenoise,
        Block::Texture,
        Block::PaperCorrection,
        Block::SourceBalance,
        Block::FlatField,
    ];

    pub fn from_index(i: i32) -> Option<Block> {
        Self::ALL.get(usize::try_from(i).ok()?).copied()
    }

    pub fn fields(self) -> &'static [Field] {
        use Field::*;
        match self {
            Block::Color => &[Temperature, Tint, ColorDensity],
            Block::Lighting => &[Exposure, Contrast, Highlights, Shadows, WhitePoint, BlackPoint],
            Block::Effects => &[Vignette, Chrome],
            Block::DensityCeiling => &[DensityCeiling],
            Block::DyeSeparation => &[DyeSeparation],
            Block::Sharpening => &[SharpenAmount, SharpenRadius, SharpenHalo],
            Block::ChromaDenoise => &[ChromaAmount, ChromaRadius],
            Block::Texture => &[Texture],
            Block::PaperCorrection => &[PaperRed, PaperGreen, PaperBlue],
            Block::SourceBalance => &[GainRed, GainGreen, GainBlue, GainTemperature, GainTint],
            Block::FlatField => &[FlatAmount, FlatTemperature, FlatTint],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub label: &'static str,
    pub range: (f32, f32),
    /// `DSSlider.centre`: the value at the middle of a bent track.
    pub centre: Option<f32>,
    pub readout: Readout,
    /// The colour axis its track runs along, whose end the readout names before its number.
    pub axis: Option<Axis>,
    /// The track runs the other way: right lifts the black, the stored value falls.
    pub inverted: bool,
}

impl Field {
    pub const ALL: [Field; 32] = [
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
    ];

    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|f| *f == self).unwrap_or(0) as i32
    }

    pub fn from_index(i: i32) -> Option<Field> {
        Self::ALL.get(usize::try_from(i).ok()?).copied()
    }

    /// The Slint `Store` property stem, `exposure` for `Store.exposure-fraction`.
    pub fn slint_name(self) -> &'static str {
        match self {
            Field::Temperature => "temperature",
            Field::Tint => "tint",
            Field::ColorDensity => "color-density",
            Field::Exposure => "exposure",
            Field::Highlights => "highlights",
            Field::Shadows => "shadows",
            Field::WhitePoint => "white-point",
            Field::BlackPoint => "black-point",
            Field::Contrast => "contrast",
            Field::Vignette => "vignette",
            Field::Chrome => "chrome",
            Field::BalanceStrength => "balance-strength",
            Field::DensityCeiling => "density-ceiling",
            Field::DyeSeparation => "dye-separation",
            Field::Angle => "angle",
            Field::SharpenAmount => "sharpen-amount",
            Field::SharpenRadius => "sharpen-radius",
            Field::SharpenHalo => "sharpen-halo",
            Field::ChromaAmount => "chroma-amount",
            Field::ChromaRadius => "chroma-radius",
            Field::Texture => "texture",
            Field::PaperRed => "paper-red",
            Field::PaperGreen => "paper-green",
            Field::PaperBlue => "paper-blue",
            Field::GainRed => "gain-red",
            Field::GainGreen => "gain-green",
            Field::GainBlue => "gain-blue",
            Field::FlatAmount => "flat-amount",
            Field::FlatTemperature => "flat-temperature",
            Field::FlatTint => "flat-tint",
            Field::GainTemperature => "gain-temperature",
            Field::GainTint => "gain-tint",
        }
    }

    pub fn block(self) -> Option<Block> {
        Block::ALL.into_iter().find(|b| b.fields().contains(&self))
    }

    /// The panel's spec for this value on a mosaic colour negative, the only mode the shell opens.
    pub fn spec(self) -> Spec {
        let s = |label, range, readout| Spec { label, range, centre: None, readout, axis: None, inverted: false };
        let warmth = |spec: Spec| Spec { axis: Some(Axis::Warmth), ..spec };
        let tint = |spec: Spec| Spec { axis: Some(Axis::Tint), ..spec };
        match self {
            // The print filters' gap in stops: red against blue, then magenta against green.
            Field::Temperature => warmth(s("Temperature", Print::TEMPERATURE_RANGE, Readout::Stops)),
            Field::Tint => tint(s("Tint", Print::TINT_RANGE, Readout::Stops)),
            Field::ColorDensity => s("Saturation", color_density::RANGE, Readout::SignedPercent),
            Field::Exposure => s("Exposure", Print::EXPOSURE_RANGE, Readout::Stops),
            Field::Highlights => s("Highlights", Print::QUARTER_RANGE, Readout::SignedPercent),
            Field::Shadows => s("Shadows", Print::QUARTER_RANGE, Readout::SignedPercent),
            Field::WhitePoint => s("White point", Print::POINT_RANGE, Readout::SignedPercent),
            Field::BlackPoint => Spec {
                inverted: true,
                ..s("Black point", Print::BLACK_POINT_RANGE, Readout::SignedPercent)
            },
            Field::Contrast => s("Contrast", Print::CONTRAST_RANGE, Readout::SignedPercent),
            Field::Vignette => s("Vignette", Vignette::RANGE, Readout::Stops),
            Field::Chrome => s("Chrome", SAUCE_RANGE, Readout::Percent),
            Field::BalanceStrength => s("Balance strength", oni_settings::blocks::filter_balance::STRENGTH_RANGE, Readout::Percent),
            Field::DensityCeiling => {
                let range = oni_settings::axis::density_axis::mosaic_range();
                s("Ceiling", (*range.start(), *range.end()), Readout::Ceiling)
            }
            Field::DyeSeparation => s("Dye separation", separation::RANGE, Readout::Percent),
            Field::Angle => s("Angle", oni_settings::frame::Geometry::ANGLE_RANGE, Readout::Degrees),
            Field::SharpenAmount => s("Amount", Sharpen::AMOUNT_RANGE, Readout::Times),
            Field::SharpenRadius => Spec {
                centre: Some(Sharpen::RADIUS_ANCHOR),
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
        }
    }

    /// The value's one rest on this frame: where its track's tick stands, where a double click lands
    /// and what its block's reset writes, the frame's import state in its own mode and decode.
    pub fn base(self, settings: &oni_settings::PipelineSettings) -> f32 {
        let mut rest = oni_settings::PipelineSettings::rest(settings.mode, settings.decoded_from_mosaic);
        if self.block() == Some(Block::SourceBalance) {
            // `restingGains`: a measured roll rests on its window's stops; a slide, on its own measure's.
            rest = settings.clone();
            rest.gains.stops = settings.roll_reference.as_ref().map(|r| r.stops).or(settings.measured_gains).unwrap_or([0.0; 3]);
        }
        self.knob().get(&rest)
    }

    /// The store's knob this slider moves.
    pub fn knob(self) -> Knob {
        match self {
            Field::Temperature => Knob::Temperature,
            Field::Tint => Knob::Tint,
            Field::ColorDensity => Knob::Density,
            Field::Exposure => Knob::Exposure,
            Field::Highlights => Knob::Highlights,
            Field::Shadows => Knob::Shadows,
            Field::WhitePoint => Knob::WhitePoint,
            Field::BlackPoint => Knob::BlackPoint,
            Field::Contrast => Knob::Contrast,
            Field::Vignette => Knob::Vignette,
            Field::Chrome => Knob::Chrome,
            Field::BalanceStrength => Knob::BalanceStrength,
            Field::DensityCeiling => Knob::DensityCeiling,
            Field::DyeSeparation => Knob::Separation,
            Field::Angle => Knob::Angle,
            Field::SharpenAmount => Knob::SharpenAmount,
            Field::SharpenRadius => Knob::SharpenRadius,
            Field::SharpenHalo => Knob::SharpenHalo,
            Field::ChromaAmount => Knob::ChromaForce,
            Field::ChromaRadius => Knob::ChromaRadius,
            Field::Texture => Knob::Texture,
            Field::PaperRed => Knob::Paper(Channel::Red),
            Field::PaperGreen => Knob::Paper(Channel::Green),
            Field::PaperBlue => Knob::Paper(Channel::Blue),
            Field::GainRed => Knob::Gain(Channel::Red),
            Field::GainGreen => Knob::Gain(Channel::Green),
            Field::GainBlue => Knob::Gain(Channel::Blue),
            Field::FlatAmount => Knob::FlatFieldAmount,
            Field::FlatTemperature => Knob::FlatFieldTemperature,
            Field::FlatTint => Knob::FlatFieldTint,
            Field::GainTemperature => Knob::GainTemperature,
            Field::GainTint => Knob::GainTint,
        }
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
