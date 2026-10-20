//! The panel's sliders for the phone: the rows the Roll, Color and Details tabs show on the open
//! face, written by the panel's own specs, and a step or reset landing as the panel's own command.

use crate::fields::Field;
use crate::slider_track::{position_on, track_on};
use oni_core::link::Sliders;
use oni_core::{Command, EditorStore};
use oni_link::{Param, Row, Tab};
use oni_settings::frame::FlatFieldMask;
use oni_settings::{Face, PipelineSettings};

/// What the panel shows beyond the settings: whether the export proofs on a paper.
pub struct PhoneSliders {
    pub on_paper: bool,
}

pub fn field(param: Param) -> Field {
    match param {
        Param::Temperature => Field::Temperature,
        Param::Tint => Field::Tint,
        Param::ColorDensity => Field::ColorDensity,
        Param::Exposure => Field::Exposure,
        Param::Highlights => Field::Highlights,
        Param::Shadows => Field::Shadows,
        Param::WhitePoint => Field::WhitePoint,
        Param::BlackPoint => Field::BlackPoint,
        Param::Contrast => Field::Contrast,
        Param::Vignette => Field::Vignette,
        Param::Chrome => Field::Chrome,
        Param::BalanceStrength => Field::BalanceStrength,
        Param::DensityCeiling => Field::DensityCeiling,
        Param::DyeSeparation => Field::DyeSeparation,
        Param::SharpenAmount => Field::SharpenAmount,
        Param::SharpenRadius => Field::SharpenRadius,
        Param::SharpenHalo => Field::SharpenHalo,
        Param::ChromaAmount => Field::ChromaAmount,
        Param::ChromaRadius => Field::ChromaRadius,
        Param::Texture => Field::Texture,
        Param::PaperRed => Field::PaperRed,
        Param::PaperGreen => Field::PaperGreen,
        Param::PaperBlue => Field::PaperBlue,
        Param::GainRed => Field::GainRed,
        Param::GainGreen => Field::GainGreen,
        Param::GainBlue => Field::GainBlue,
        Param::FlatAmount => Field::FlatAmount,
        Param::FlatTemperature => Field::FlatTemperature,
        Param::FlatTint => Field::FlatTint,
    }
}

/// The panel's blocks of sliders in its order, each with the tab and title it stands under, as the
/// Slint panes lay them on this face; a contact sheet's grain blocks are out of reach there.
pub fn rows(s: &PipelineSettings, face: Face, on_paper: bool) -> Vec<Row> {
    use Param::*;
    let mono = s.mode.is_monochrome();
    let pro = face == Face::Pro;
    let flat = &s.flat_field;
    // `FlatFieldBlock`: the sliders act once a mask is chosen.
    let masked = flat.mask != FlatFieldMask::None && !(flat.mask == FlatFieldMask::Custom && flat.custom.is_none());
    let separation = !mono && s.rgb_passes.is_none() && pro;
    let sheet = s.sheet.is_some();
    let blocks: [(Tab, &str, bool, &[Param]); 12] = [
        (Tab::Roll, "Flat-Field", masked, if mono { &[FlatAmount] } else { &[FlatAmount, FlatTemperature, FlatTint] }),
        (Tab::Roll, "Source Balance", true, &[GainRed, GainGreen, GainBlue]),
        (Tab::Roll, "Density Ceiling", true, &[DensityCeiling]),
        (Tab::Roll, "Dye Separation", separation, &[DyeSeparation]),
        (Tab::Roll, "Balance Strength", separation, &[BalanceStrength]),
        (Tab::Color, "Color", pro && !mono, &[Temperature, Tint, ColorDensity]),
        (Tab::Color, "Paper Correction", on_paper, &[PaperRed, PaperGreen, PaperBlue]),
        (Tab::Color, "Lighting", pro, &[Exposure, Highlights, Shadows, WhitePoint, BlackPoint]),
        (Tab::Color, "Effects", pro, if mono { &[Contrast, Vignette] } else { &[Contrast, Vignette, Chrome] }),
        (Tab::Details, "Sharpening", !sheet, &[SharpenAmount, SharpenRadius, SharpenHalo]),
        (Tab::Details, "Chroma Denoise", !sheet && !mono, &[ChromaAmount, ChromaRadius]),
        (Tab::Details, "Texture", true, &[Texture]),
    ];
    let inverted = s.mode.inverts_gain_sense();
    let mut out = Vec::new();
    for (tab, block, shown, params) in blocks {
        if !shown {
            continue;
        }
        for &param in params {
            let f = field(param);
            let v = f.knob().get(s);
            out.push(Row {
                param,
                tab,
                block: block.to_string(),
                label: label(f, inverted),
                value: f.readout(v),
                position: position_on(f, v, &track_on(f, s)),
                modified: (v - f.double_click_base(s)).abs() > 1e-6,
            });
        }
    }
    out
}

/// `GainPanel`'s rows named by their two ends, swapped by the inversion; every other slider its spec's.
fn label(f: Field, inverted: bool) -> String {
    let ends = match f {
        Field::GainRed => ("Cyan ↔ Red", "Red ↔ Cyan"),
        Field::GainGreen => ("Magenta ↔ Green", "Green ↔ Magenta"),
        Field::GainBlue => ("Yellow ↔ Blue", "Blue ↔ Yellow"),
        _ => return f.spec().label.to_string(),
    };
    (if inverted { ends.1 } else { ends.0 }).to_string()
}

/// A step moves the knob a hundredth of its track to the right per click, the shown sense, so a
/// black point's plus lifts it as the knob goes; a reset is the double click's.
pub fn command(s: &PipelineSettings, param: Param, by: Option<i8>) -> Command {
    let f = field(param);
    let value = match by {
        Some(by) => {
            let (low, high) = f.spec().range;
            let sense = if f.spec().inverted { -1.0 } else { 1.0 };
            f.knob().get(s) + sense * f32::from(by) * (high - low) / 100.0
        }
        None => f.double_click_base(s),
    };
    Command::Set { knob: f.knob(), value }
}

impl Sliders for PhoneSliders {
    fn rows(&self, store: &EditorStore) -> Vec<Row> {
        if store.path().is_none() {
            return Vec::new();
        }
        rows(store.settings(), store.face(), self.on_paper)
    }

    /// Only a slider the panel shows now moves, as a hand finds only those.
    fn command(&self, store: &EditorStore, param: Param, by: Option<i8>) -> Option<Command> {
        let shown = self.rows(store).iter().any(|r| r.param == param);
        shown.then(|| command(store.settings(), param, by))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::ConversionMode;

    fn params(rows: &[Row], tab: Tab) -> Vec<Param> {
        rows.iter().filter(|r| r.tab == tab).map(|r| r.param).collect()
    }

    #[test]
    fn each_face_lists_the_sliders_its_tabs_show() {
        let s = PipelineSettings::rest(ConversionMode::Negative, true);
        let pro = rows(&s, Face::Pro, false);
        use Param::*;
        assert_eq!(params(&pro, Tab::Roll), [GainRed, GainGreen, GainBlue, DensityCeiling, DyeSeparation, BalanceStrength]);
        assert_eq!(params(&pro, Tab::Color), [Temperature, Tint, ColorDensity, Exposure, Highlights, Shadows, WhitePoint, BlackPoint, Contrast, Vignette, Chrome]);
        assert_eq!(params(&pro, Tab::Details), [SharpenAmount, SharpenRadius, SharpenHalo, ChromaAmount, ChromaRadius, Texture]);
        let exposure = pro.iter().find(|r| r.param == Exposure).unwrap();
        assert_eq!((exposure.block.as_str(), exposure.label.as_str()), ("Lighting", "Exposure"));
        assert_eq!(exposure.value, Field::Exposure.readout(s.print.exposure));
        assert_eq!(pro[0].label, "Red ↔ Cyan", "a negative's gains read inverted");
        // Twin: the Scanner face leaves Color to its keypad, and the roll's dye blocks out.
        let scanner = rows(&s, Face::Scanner, true);
        assert_eq!(params(&scanner, Tab::Color), [PaperRed, PaperGreen, PaperBlue]);
        assert_eq!(params(&scanner, Tab::Roll), [GainRed, GainGreen, GainBlue, DensityCeiling]);
        assert_eq!(params(&scanner, Tab::Details), params(&pro, Tab::Details));
    }

    #[test]
    fn black_and_white_drops_the_colour_sliders() {
        let s = PipelineSettings::rest(ConversionMode::Monochrome, true);
        let rows = rows(&s, Face::Pro, false);
        for gone in [Param::Temperature, Param::Chrome, Param::ChromaAmount, Param::DyeSeparation] {
            assert!(!rows.iter().any(|r| r.param == gone), "{gone:?}");
        }
        // Twin: the sliders a monochrome frame keeps are still there.
        assert!(rows.iter().any(|r| r.param == Param::Contrast) && rows.iter().any(|r| r.param == Param::Texture));
    }

    #[test]
    fn a_step_is_a_hundredth_of_the_track_rightward_and_a_reset_the_double_click() {
        let s = PipelineSettings::rest(ConversionMode::Negative, true);
        let (low, high) = Field::Exposure.spec().range;
        let Command::Set { value, .. } = command(&s, Param::Exposure, Some(3)) else { panic!() };
        assert!((value - s.print.exposure - 3.0 * (high - low) / 100.0).abs() < 1e-6);
        // The black point's track runs the other way: its plus lowers the stored value.
        let Command::Set { value, .. } = command(&s, Param::BlackPoint, Some(1)) else { panic!() };
        assert!(value < s.print.black_point);
        let mut moved = s.clone();
        moved.print.exposure = 0.7;
        let Command::Set { value, .. } = command(&moved, Param::Exposure, None) else { panic!() };
        assert_eq!(value, Field::Exposure.double_click_base(&moved));
        // Twin: a step on another slider leaves Exposure's knob, so the check reads the right one.
        let Command::Set { knob, .. } = command(&s, Param::Texture, Some(1)) else { panic!() };
        assert_ne!(knob, Field::Exposure.knob());
    }

    #[test]
    fn every_slider_of_the_link_is_a_panel_field_of_its_own() {
        let fields: std::collections::HashSet<Field> = Param::ALL.iter().map(|p| field(*p)).collect();
        assert_eq!(fields.len(), Param::ALL.len());
        // Twin: the only panel field the phone has no slider for is the crop's angle.
        let missing: Vec<Field> = Field::ALL.into_iter().filter(|f| !fields.contains(f)).collect();
        assert_eq!(missing, [Field::Angle]);
    }
}
