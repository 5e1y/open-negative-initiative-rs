//! Every value the panel draws: its label, travel, rest, readout and the store knob it moves, copied
//! from the Swift panes (`ColorPane`, `PrintPanel`, `SettingsPane`, `DetailsPane`, …).

use oni_core::knob::{CHROMA_FORCE_RANGE, Knob, SAUCE_RANGE};
use oni_settings::blocks::{ChromaDenoise, Sharpen, Texture, Vignette, separation};
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
}

impl Block {
    pub const ALL: [Block; 9] = [
        Block::Color,
        Block::Lighting,
        Block::Effects,
        Block::BalanceStrength,
        Block::DensityCeiling,
        Block::DyeSeparation,
        Block::Sharpening,
        Block::ChromaDenoise,
        Block::Texture,
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
    pub const ALL: [Field; 21] = [
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
            Field::DyeSeparation => s("Dye separation", separation::RANGE, separation::BASE, Readout::Plain),
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
        }
    }

    /// What the track shows for a stored value: flipped where the track runs the other way.
    pub fn shown(self, stored: f32) -> f32 {
        if self.spec().inverted { -stored } else { stored }
    }

    /// The range the track spans, in the shown sense.
    pub fn track(self) -> (f32, f32) {
        let spec = self.spec();
        if spec.inverted { (-spec.range.1, -spec.range.0) } else { spec.range }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_readouts_print_as_the_swift_formats_do() {
        assert_eq!(Field::Exposure.spec().readout.format(1.5), "+1.50 EV");
        assert_eq!(Field::ColorDensity.spec().readout.format(-0.25), "-25 %");
        assert_eq!(Field::Temperature.spec().readout.format(1.0), "+50");
        assert_eq!(Field::Tint.spec().readout.format(-1.5), "-100");
        assert_eq!(Field::Chrome.spec().readout.format(0.5), "50%");
        assert_eq!(Field::BalanceStrength.spec().readout.format(0.67), "67 %");
        assert_eq!(Field::SharpenRadius.spec().readout.format(2.092), "2.1 px");
        assert_eq!(Field::Angle.spec().readout.format(-1.0), "-1.00°");
        // Twin: Temperature's readout is a share of its top, not the stored stops.
        assert_ne!(Field::Temperature.spec().readout.format(1.0), Readout::Signed.format(1.0));
    }

    #[test]
    fn every_field_has_its_place_in_the_slint_store() {
        let store = include_str!("../ui/store.slint");
        for f in Field::ALL {
            assert_eq!(Field::from_index(f.index()), Some(f));
            for suffix in ["fraction", "readout", "value"] {
                assert!(store.contains(&format!("{}-{suffix};", f.slint_name())), "{f:?} has no {suffix}");
            }
        }
        // The panel passes each slider's own index back.
        let panel = include_str!("../ui/panel.slint");
        for f in Field::ALL {
            let wired = format!("field: {}; label: \"{}\"; fraction: Store.{}-fraction;", f.index(), f.spec().label, f.slint_name());
            assert!(panel.contains(&wired), "{f:?} is not wired as {wired}");
        }
        // Twin: a field the store never declared is caught.
        assert!(!store.contains("grain-fraction;"));
    }

    #[test]
    fn each_slider_spans_its_knobs_range_on_a_mosaic_negative() {
        let s = oni_settings::PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
        for f in Field::ALL {
            assert_eq!(f.knob().range(&s), Some(f.spec().range), "{f:?}");
        }
        // Twin: on a plain file the ceiling's axis is another, which the spec does not claim to follow.
        let plain = oni_settings::PipelineSettings::rest(oni_settings::ConversionMode::Negative, false);
        assert_ne!(Field::DensityCeiling.knob().range(&plain), Some(Field::DensityCeiling.spec().range));
    }

    #[test]
    fn the_black_point_runs_the_other_way() {
        let f = Field::BlackPoint;
        assert_eq!(f.track(), (-Print::BLACK_POINT_RANGE.1, -Print::BLACK_POINT_RANGE.0));
        assert_eq!(f.shown(0.1), -0.1);
        assert_eq!(Field::WhitePoint.shown(0.1), 0.1);
    }
}
