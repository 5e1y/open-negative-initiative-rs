//! The panel's sliders for the phone: the rows the Roll, Color and Details tabs show on the open
//! face, written by the panel's own specs, and a step or reset landing as the panel's own command.

use crate::fields::Field;
use crate::slider_track::{position_on, stored_on, track_on};
use crate::track;
use crate::link::{Move, Sliders};
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
    let blocks: [(Tab, &str, bool, &[Param]); 11] = [
        (Tab::Roll, "Flat-Field", masked, if mono { &[FlatAmount] } else { &[FlatAmount, FlatTemperature, FlatTint] }),
        (Tab::Roll, "Filtration", true, &[GainRed, GainGreen, GainBlue]),
        (Tab::Roll, "Highlight Detail", true, &[DensityCeiling]),
        (Tab::Roll, "Dye Separation", separation, &[DyeSeparation]),
        (Tab::Color, "Lighting", pro, &[Exposure, Contrast, Highlights, Shadows, WhitePoint, BlackPoint]),
        (Tab::Color, "Color", pro && !mono, &[BalanceStrength, Temperature, Tint, ColorDensity]),
        (Tab::Color, "Paper Correction", on_paper, &[PaperRed, PaperGreen, PaperBlue]),
        (Tab::Color, "Effects", pro, if mono { &[Vignette] } else { &[Vignette, Chrome] }),
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

/// On the open face's view `s`: a step moves a hundredth of the track rightward per click, the shown
/// sense, a place on the track is the panel's drag, both moves; a reset is the double click's.
pub fn command(s: &PipelineSettings, param: Param, by: Move) -> Command {
    let f = field(param);
    let knob = f.knob();
    match by {
        Move::Step(by) => {
            let (low, high) = f.spec().range;
            let sense = if f.spec().inverted { -1.0 } else { 1.0 };
            Command::Step { knob, by: sense * f32::from(by) * (high - low) / 100.0 }
        }
        Move::Reset => Command::Set { knob, value: f.double_click_base(s) },
        Move::To(at) => Command::Step { knob, by: stored_on(f, track::clamped(at), &track_on(f, s)) - knob.get(s) },
    }
}

impl Sliders for PhoneSliders {
    fn rows(&self, store: &EditorStore) -> Vec<Row> {
        if store.path().is_none() {
            return Vec::new();
        }
        rows(store.settings(), store.face(), self.on_paper)
    }

    /// Only a slider the panel shows now moves, as a hand finds only those.
    fn command(&self, store: &EditorStore, param: Param, by: Move) -> Option<Command> {
        let shown = self.rows(store).iter().any(|r| r.param == param);
        shown.then(|| command(&store.face_view(), param, by))
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
        assert_eq!(params(&pro, Tab::Roll), [GainRed, GainGreen, GainBlue, DensityCeiling, DyeSeparation]);
        assert_eq!(params(&pro, Tab::Color), [Exposure, Contrast, Highlights, Shadows, WhitePoint, BlackPoint, BalanceStrength, Temperature, Tint, ColorDensity, Vignette, Chrome]);
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

    /// The knob's value once `command` lands on `s`.
    fn landed(s: &PipelineSettings, command: Command) -> f32 {
        match command {
            Command::Step { knob, by } => knob.get(&knob.set(s, knob.get(s) + by)),
            Command::Set { knob, value } => knob.get(&knob.set(s, value)),
            _ => panic!("a knob's command"),
        }
    }

    #[test]
    fn a_step_is_a_hundredth_of_the_track_rightward_and_a_reset_the_double_click() {
        let s = PipelineSettings::rest(ConversionMode::Negative, true);
        let (low, high) = Field::Exposure.spec().range;
        let Command::Step { by, .. } = command(&s, Param::Exposure, Move::Step(3)) else { panic!() };
        assert!((by - 3.0 * (high - low) / 100.0).abs() < 1e-6);
        // The black point's track runs the other way: its plus lowers the stored value.
        let Command::Step { by, .. } = command(&s, Param::BlackPoint, Move::Step(1)) else { panic!() };
        assert!(by < 0.0);
        let mut moved = s.clone();
        moved.print.exposure = 0.7;
        let Command::Set { value, .. } = command(&moved, Param::Exposure, Move::Reset) else { panic!() };
        assert_eq!(value, Field::Exposure.double_click_base(&moved));
        // Twin: a step on another slider leaves Exposure's knob, so the check reads the right one.
        let Command::Step { knob, .. } = command(&s, Param::Texture, Move::Step(1)) else { panic!() };
        assert_ne!(knob, Field::Exposure.knob());
    }

    #[test]
    fn a_place_on_the_track_is_where_the_panel_s_drag_puts_the_knob() {
        let mono = PipelineSettings::rest(ConversionMode::Monochrome, true);
        for (s, param) in [(&mono, Param::Contrast), (&mono, Param::BlackPoint), (&mono, Param::Exposure)] {
            let f = field(param);
            let placed = command(s, param, Move::To(0.8));
            assert!(matches!(placed, Command::Step { knob, .. } if knob == f.knob()));
            assert!((position_on(f, landed(s, placed), &track_on(f, s)) - 0.8).abs() < 1e-4, "{param:?}");
        }
        // Twin: a monochrome frame's contrast spans its own track, so the fixed range's place is another value.
        let value = landed(&mono, command(&mono, Param::Contrast, Move::To(0.8)));
        assert!((value - crate::slider_track::stored_at(Field::Contrast, 0.8)).abs() > 1e-3);
        let at = |p| landed(&mono, command(&mono, Param::Exposure, Move::To(p)));
        assert_eq!(at(1.7), at(1.0), "past the end, the end");
    }

    #[test]
    fn the_phone_moves_each_frame_together_by_its_distance() {
        let dir = std::env::temp_dir().join(format!("oni-app-phone-together-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let [a, b] = ["a.tif", "b.tif"].map(|n| dir.join(n).to_string_lossy().into_owned());
        let mut own = PipelineSettings::rest(ConversionMode::Negative, false);
        own.print.exposure = -0.6;
        for p in [&a, &b] {
            std::fs::write(p, b"").unwrap();
        }
        oni_settings::sidecar::write(&own, std::path::Path::new(&b), Default::default()).unwrap();
        let mut store = EditorStore::new(oni_core::Archive::new(vec![a.clone(), b.clone()], |_: &std::path::Path| false));
        store.open(&a).unwrap();
        store.set_together(vec![b.clone()]);
        let open = store.settings().print.exposure;
        let phone = PhoneSliders { on_paper: false };
        for by in [Move::Step(2), Move::To(0.7)] {
            let command = phone.command(&store, Param::Exposure, by).unwrap();
            store.apply(command);
        }
        let moved = store.settings().print.exposure - open;
        let read = oni_settings::sidecar::read(std::path::Path::new(&b), false, true).unwrap().settings;
        assert!(moved.abs() > 0.05 && (read.print.exposure - (-0.6 + moved)).abs() < 1e-5, "{moved} {}", read.print.exposure);
        // Twin: the open frame's value laid whole would have wiped the frame's own offset.
        assert!((read.print.exposure - store.settings().print.exposure).abs() > 0.1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_slider_of_the_link_is_a_panel_field_of_its_own() {
        let fields: std::collections::HashSet<Field> = Param::ALL.iter().map(|p| field(*p)).collect();
        assert_eq!(fields.len(), Param::ALL.len());
        // Twin: the phone has no slider for the crop's angle nor a slide's gains temperature and tint.
        let missing: Vec<Field> = Field::ALL.into_iter().filter(|f| !fields.contains(f)).collect();
        assert_eq!(missing, [Field::Angle, Field::GainTemperature, Field::GainTint]);
    }
}
