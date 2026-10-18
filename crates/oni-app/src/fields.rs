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
            // `restingGains`: a measured roll is the frame's rest, its window's stops.
            Field::GainRed | Field::GainGreen | Field::GainBlue => {
                let i = [Field::GainRed, Field::GainGreen, Field::GainBlue].iter().position(|f| *f == self).unwrap_or(0);
                settings.roll_reference.as_ref().map_or(0.0, |r| r.stops[i])
            }
            _ => self.spec().base,
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
        let store = include_str!("../../oni-ui/ui/store.slint");
        for f in Field::ALL {
            assert_eq!(Field::from_index(f.index()), Some(f));
            for suffix in ["fraction", "readout", "value"] {
                assert!(store.contains(&format!("{}-{suffix};", f.slint_name())), "{f:?} has no {suffix}");
            }
        }
        // The panel passes each slider's own index back.
        let panel = panel_source();
        for f in Field::ALL {
            // Source Balance names each row by the ends the mode gives it, so its label is the panel's.
            let named = |l: &str| l.contains(&format!("label: \"{}\";", f.spec().label)) || f.block() == Some(Block::SourceBalance);
            let wired = panel.lines().any(|l| {
                l.contains(&format!("field: {}; label: ", f.index())) && l.contains(&format!("fraction: Store.{}-fraction;", f.slint_name())) && named(l)
            });
            assert!(wired, "{f:?} is not wired by its index, label and fraction");
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
    fn a_double_click_on_the_ceiling_lands_on_the_frames_own_rest() {
        use oni_settings::{ConversionMode, PipelineSettings};
        let slide = PipelineSettings::rest(ConversionMode::Positive, true);
        assert_eq!(Field::DensityCeiling.base(&slide), slide.density_ceiling);
        let negative = PipelineSettings::rest(ConversionMode::Negative, true);
        assert_eq!(Field::DensityCeiling.base(&negative), negative.density_ceiling);
        // Twin: the two rests differ, so a base read for the negative alone fails on the slide.
        assert_ne!(slide.density_ceiling, Field::DensityCeiling.spec().base);
        assert_eq!(Field::Exposure.base(&slide), 0.0, "Exposure's base is `Tone.neutral`, never the mode's rest");
    }

    /// The panel's Slint, its tabs' files after it.
    pub(crate) fn panel_source() -> String {
        [
            include_str!("../../oni-ui/ui/panel.slint"),
            include_str!("../../oni-ui/ui/panel/state.slint"),
            include_str!("../../oni-ui/ui/panel/parts.slint"),
            include_str!("../../oni-ui/ui/panel/roll_tab.slint"),
            include_str!("../../oni-ui/ui/panel/crop_tab.slint"),
            include_str!("../../oni-ui/ui/panel/dedust_tab.slint"),
            include_str!("../../oni-ui/ui/panel/color_tab.slint"),
            include_str!("../../oni-ui/ui/panel/details_tab.slint"),
        ]
        .concat()
    }

    /// The `title:` of each block in one pane of the panel, in order.
    fn pane_titles(panel: &str, pane: &str) -> Vec<String> {
        let body = &panel[panel.find(&format!("component {pane} ")).expect("the pane")..];
        let next = ["\ncomponent ", "\nexport component "].iter().filter_map(|m| body[1..].find(m)).min();
        let body = &body[..next.map_or(body.len(), |i| i + 1)];
        body.lines().filter_map(|l| l.trim().strip_prefix("title: \"")?.split('"').next().map(str::to_string)).collect()
    }

    /// Whether `shown` keeps `order`'s relative order, every one of its blocks found there.
    fn follows(shown: &[String], order: &[String]) -> bool {
        let mut at = 0;
        shown.iter().all(|t| match order[at..].iter().position(|o| o == t) {
            Some(i) => {
                at += i + 1;
                true
            }
            None => false,
        })
    }

    /// `SettingsPane.body`'s sections as drawn: `manualBalance` is a property laid where the body
    /// names it, though its text comes later in the file.
    fn swift_roll_order(text: &str) -> Vec<String> {
        let sections = |t: &str| -> Vec<String> { t.split("DSSection(\"").skip(1).filter_map(|t| t.split('"').next().map(str::to_string)).collect() };
        let body = &text[text.find("var body").expect("body")..text.find("private var rollBlock").expect("rollBlock")];
        let manual = &text[text.find("private var manualBalance").expect("manualBalance")..text.find("// MARK: - Keys").expect("keys")];
        let at = body.find("manualBalance\n").expect("the body names manualBalance");
        [sections(&body[..at]), sections(manual), sections(&body[at..])].concat()
    }

    #[test]
    fn the_roll_tab_lays_its_blocks_in_the_swift_order() {
        // `SettingsPane.body`, read from the Swift source when it is beside this checkout.
        let swift: Vec<String> = match crate::swift_sources().and_then(|d| std::fs::read_to_string(d.join("UI/SettingsPane.swift")).ok()) {
            Some(text) => swift_roll_order(&text),
            None => ["Measured Roll", "Film Type", "Flat-Field", "Source Encoding", "Source Balance", "Density Ceiling", "Per-Channel Levels", "Dye Separation", "Balance Strength"]
                .map(String::from)
                .to_vec(),
        };
        let shown = pane_titles(&panel_source(), "RollPane");
        assert!(shown.len() >= 6, "{shown:?}");
        assert!(follows(&shown, &swift), "the Roll tab shows {shown:?}, Swift lays {swift:?}");
        // Twin: Dye Separation and Balance Strength ahead of the ceiling break the order.
        let early: Vec<String> = ["Measured Roll", "Film Type", "Flat-Field", "Dye Separation", "Balance Strength", "Density Ceiling"].map(String::from).to_vec();
        assert!(!follows(&early, &swift));
    }

    #[test]
    fn the_black_point_runs_the_other_way() {
        let f = Field::BlackPoint;
        assert_eq!(f.track(), (-Print::BLACK_POINT_RANGE.1, -Print::BLACK_POINT_RANGE.0));
        assert_eq!(f.shown(0.1), -0.1);
        assert_eq!(Field::WhitePoint.shown(0.1), 0.1);
    }
}
