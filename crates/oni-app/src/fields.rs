//! Every value the panel draws: its label, travel, rest, readout and the store knob it moves, copied
//! from the Swift panes (`ColorPane`, `PrintPanel`, `SettingsPane`, `DetailsPane`, …).

use oni_core::knob::{CHROMA_FORCE_RANGE, Channel, Knob, SAUCE_RANGE};
use oni_settings::blocks::{ChromaDenoise, Gains, PaperCorrection, Sharpen, Texture, Vignette, separation};
use oni_settings::frame::FlatField;
use oni_settings::tone::{Print, color_density};

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
}

/// The blocks a reset key clears, in the order of the Slint `Store`'s `*-modified` flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Block {
    Color,
    Lighting,
    Effects,
    BalanceStrength,
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
    pub const ALL: [Block; 12] = [
        Block::Color,
        Block::Lighting,
        Block::Effects,
        Block::BalanceStrength,
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
            Block::Lighting => &[Exposure, Highlights, Shadows, WhitePoint, BlackPoint],
            Block::Effects => &[Contrast, Vignette, Chrome],
            Block::BalanceStrength => &[BalanceStrength],
            Block::DensityCeiling => &[DensityCeiling],
            Block::DyeSeparation => &[DyeSeparation],
            Block::Sharpening => &[SharpenAmount, SharpenRadius, SharpenHalo],
            Block::ChromaDenoise => &[ChromaAmount, ChromaRadius],
            Block::Texture => &[Texture],
            Block::PaperCorrection => &[PaperRed, PaperGreen, PaperBlue],
            Block::SourceBalance => &[GainRed, GainGreen, GainBlue],
            Block::FlatField => &[FlatAmount, FlatTemperature, FlatTint],
        }
    }
}

/// How a slider writes its value: as `String(format:)` does in each Swift pane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Readout {
    /// `%+.2f EV`
    Stops,
    /// `%+.2f`
    Signed,
    /// `%+.0f %%` of the value times 100.
    SignedPercentSpaced,
    /// `%.0f %%` of the value times 100.
    PercentSpaced,
    /// `%.0f%%` of the value times 100.
    Percent,
    /// `%+.0f` of the value over the range's top, times 100.
    OfRange(f32),
    /// `%.3f`
    Ceiling,
    /// `%.2f`
    Plain,
    /// `%+.2f°`
    Degrees,
    /// `%.2f×`
    Times,
    /// `%+.2f×`
    SignedTimes,
    /// `%.1f px`
    Pixels,
}

impl Readout {
    pub fn format(self, v: f32) -> String {
        // `String(format:)` prints −0 for a negative zero, and so would `{:+}`; Swift's `%+.2f` of −0.0 is "-0.00".
        match self {
            Readout::Stops => format!("{v:+.2} EV"),
            Readout::Signed => format!("{v:+.2}"),
            Readout::SignedPercentSpaced => format!("{:+.0} %", v * 100.0),
            Readout::PercentSpaced => format!("{:.0} %", v * 100.0),
            Readout::Percent => format!("{:.0}%", v * 100.0),
            Readout::OfRange(top) => format!("{:+.0}", v / top * 100.0),
            Readout::Ceiling => format!("{v:.3}"),
            Readout::Plain => format!("{v:.2}"),
            Readout::Degrees => format!("{v:+.2}°"),
            Readout::Times => format!("{v:.2}×"),
            Readout::SignedTimes => format!("{v:+.2}×"),
            Readout::Pixels => format!("{v:.1} px"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub label: &'static str,
    pub range: (f32, f32),
    /// The double click's rest, in the stored value's sense.
    pub base: f32,
    /// `DSSlider.centre`: the value at the middle of a bent track.
    pub centre: Option<f32>,
    pub readout: Readout,
    /// The track runs the other way: right lifts the black, the stored value falls.
    pub inverted: bool,
}

impl Field {
    pub const ALL: [Field; 30] = [
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
        }
    }

    pub fn block(self) -> Option<Block> {
        Block::ALL.into_iter().find(|b| b.fields().contains(&self))
    }

    /// The panel's spec for this value on a mosaic colour negative, the only mode the shell opens.
    pub fn spec(self) -> Spec {
        let s = |label, range, base, readout| Spec { label, range, base, centre: None, readout, inverted: false };
        match self {
            Field::Temperature => s("Temperature", Print::TEMPERATURE_RANGE, 0.0, Readout::OfRange(Print::TEMPERATURE_RANGE.1)),
            Field::Tint => s("Tint", Print::TINT_RANGE, 0.0, Readout::OfRange(Print::TINT_RANGE.1)),
            Field::ColorDensity => s(
                "Color density (same as saturation)",
                color_density::RANGE,
                color_density::BASE,
                Readout::SignedPercentSpaced),
            Field::Exposure => s("Exposure", Print::EXPOSURE_RANGE, 0.0, Readout::Stops),
            Field::Highlights => s("Highlights", Print::QUARTER_RANGE, 0.0, Readout::Signed),
            Field::Shadows => s("Shadows", Print::QUARTER_RANGE, 0.0, Readout::Signed),
            Field::WhitePoint => s("White point", Print::POINT_RANGE, 0.0, Readout::SignedPercentSpaced),
            Field::BlackPoint => Spec {
                inverted: true,
                ..s("Black point", Print::BLACK_POINT_RANGE, Print::RESTING_BLACK_POINT, Readout::SignedPercentSpaced)
            },
            Field::Contrast => s("Contrast", Print::CONTRAST_RANGE, 0.0, Readout::Signed),
            Field::Vignette => s("Vignette", Vignette::RANGE, 0.0, Readout::Stops),
            Field::Chrome => s("Chrome", SAUCE_RANGE, 0.0, Readout::Percent),
            Field::BalanceStrength => s(
                "Strength",
                oni_settings::blocks::filter_balance::STRENGTH_RANGE,
                oni_settings::blocks::filter_balance::RESTING_STRENGTH,
                Readout::PercentSpaced),
            Field::DensityCeiling => {
                let range = oni_settings::axis::density_axis::mosaic_range();
                s(
                    "Ceiling",
                    (*range.start(), *range.end()),
                    oni_settings::axis::density_axis::MOSAIC_REST,
                    Readout::Ceiling)
            }
            Field::DyeSeparation => s("Dye separation", separation::RANGE, separation::BASE, Readout::PercentSpaced),
            Field::Angle => s("Angle", oni_settings::frame::Geometry::ANGLE_RANGE, 0.0, Readout::Degrees),
            Field::SharpenAmount => s("Amount", Sharpen::AMOUNT_RANGE, 0.0, Readout::Times),
            Field::SharpenRadius => Spec {
                centre: Some(Sharpen::RADIUS_ANCHOR),
                ..s("Radius", Sharpen::radius_range(), Sharpen::RADIUS_ANCHOR, Readout::Pixels)
            },
            Field::SharpenHalo => s("Halo limit", Sharpen::HALO_RANGE, Sharpen::default().halo, Readout::Percent),
            Field::ChromaAmount => s("Amount", CHROMA_FORCE_RANGE, ChromaDenoise::default().force, Readout::Percent),
            Field::ChromaRadius => {
                s("Radius", ChromaDenoise::RADIUS_RANGE, ChromaDenoise::default().radius, Readout::Pixels)
            }
            Field::Texture => s("Texture", Texture::RANGE, 0.0, Readout::SignedTimes),
            // `GainPanel` titled by its channel alone, as the Paper Correction block names them.
            Field::PaperRed => s("Red", PaperCorrection::RANGE, 0.0, Readout::Stops),
            Field::PaperGreen => s("Green", PaperCorrection::RANGE, 0.0, Readout::Stops),
            Field::PaperBlue => s("Blue", PaperCorrection::RANGE, 0.0, Readout::Stops),
            // `GainPanel`'s rows, named by the ends the mode gives them, which the panel lays.
            Field::GainRed => s("Red", Gains::RANGE, 0.0, Readout::Stops),
            Field::GainGreen => s("Green", Gains::RANGE, 0.0, Readout::Stops),
            Field::GainBlue => s("Blue", Gains::RANGE, 0.0, Readout::Stops),
            Field::FlatAmount => s("Amount", FlatField::AMOUNT_RANGE, 1.0, Readout::Percent),
            Field::FlatTemperature => s("Temperature", FlatField::COLOUR_RANGE, 0.0, Readout::Signed),
            Field::FlatTint => s("Tint", FlatField::COLOUR_RANGE, 0.0, Readout::Signed),
        }
    }

    /// Where a double click lands on this frame: the slider's `base`, which the ceiling's reads off
    /// the frame's mode and decode as `SettingsPane` does.
    pub fn base(self, settings: &oni_settings::PipelineSettings) -> f32 {
        match self {
            Field::DensityCeiling => oni_settings::axis::density_axis::rest(settings.decoded_from_mosaic, settings.mode),
            // `restingGains`: a measured roll is the frame's rest, its window's stops; a slide's, its own measure's.
            Field::GainRed | Field::GainGreen | Field::GainBlue => {
                let i = [Field::GainRed, Field::GainGreen, Field::GainBlue].iter().position(|f| *f == self).unwrap_or(0);
                settings.roll_reference.as_ref().map(|r| r.stops[i]).or(settings.measured_gains.map(|g| g[i])).unwrap_or(0.0)
            }
            _ => self.spec().base,
        }
    }

    /// Where a double click lands, `DSSlider`'s base: a gain goes to `Gains.base`, 0, while its block's
    /// reset goes to the roll's stops (`base`).
    pub fn double_click_base(self, settings: &oni_settings::PipelineSettings) -> f32 {
        match self {
            Field::GainRed | Field::GainGreen | Field::GainBlue => self.spec().base,
            _ => self.base(settings),
        }
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
        }
    }

    /// What the track shows for a stored value: flipped where the track runs the other way.
    pub fn shown(self, stored: f32) -> f32 {
        if self.spec().inverted { -stored } else { stored }
    }

    /// The readout of a stored value, in the sense the slider shows it (`DSSlider.format(shown)`).
    pub fn readout(self, stored: f32) -> String {
        self.spec().readout.format(self.shown(stored))
    }

    /// The range the track spans, in the shown sense.
    pub fn track(self) -> (f32, f32) {
        let spec = self.spec();
        if spec.inverted { (-spec.range.1, -spec.range.0) } else { spec.range }
    }
}

#[cfg(test)]
mod tests;
