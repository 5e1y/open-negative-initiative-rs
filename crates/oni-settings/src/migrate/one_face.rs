//! The Scanner face a document before format 46 kept beside Pro: a photo it rendered keeps that edit alone, in Lighting's
//! fields and a legacy print; a photo Pro rendered keeps Pro's, the Scanner's fields going as they never rendered.

use super::chain::{DIGITAL_SCANNER_VERSION, ONE_FACE_VERSION};
use super::departure::{FORMER_REST, former_rest};
use super::exposure::GEAR;
use crate::blocks::Vignette;
use crate::curves::{CurveSet, SaturationCurve};
use crate::document::Failure;
use crate::legacy_print::{LegacyMachine, LegacyPrint};
use crate::spectrogram::Spectrogram;
use crate::tolerant;
use crate::tone::Print;
use crate::{ConversionMode, PipelineSettings};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Which edit rendered the photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Face {
    #[default]
    Pro,
    Scanner,
}

/// The keypad's steps on D, C, M and Y, then TEMP: counts, never densities, each held on its stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Clicks {
    pub d: i64,
    pub c: i64,
    pub m: i64,
    pub y: i64,
    pub w: i64,
}

impl Clicks {
    /// Steps in one unit of the keypad's screen.
    pub const PER_UNIT: i64 = LegacyPrint::PER_POINT;
    pub const DENSITY_RANGE: (i64, i64) = (-24 * Self::PER_UNIT, 24 * Self::PER_UNIT);
    pub const WARMTH_RANGE: (i64, i64) = (-10 * Self::PER_UNIT, 10 * Self::PER_UNIT);

    pub fn new(d: i64, c: i64, m: i64, y: i64, w: i64) -> Self {
        let (lo, hi) = Self::DENSITY_RANGE;
        let w = w.clamp(Self::WARMTH_RANGE.0, Self::WARMTH_RANGE.1);
        Clicks { d: d.clamp(lo, hi), c: c.clamp(lo, hi), m: m.clamp(lo, hi), y: y.clamp(lo, hi), w }
    }

    pub fn is_neutral(&self) -> bool {
        *self == Clicks::default()
    }
}

/// Tolerant of absent or unreadable keys, and a count past its range lands on its stop.
impl<'de> Deserialize<'de> for Clicks {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = tolerant::object(Value::deserialize(d)?, "ScannerClicks")?;
        let read = |key: &str| tolerant::integer(map.get(key)).unwrap_or(0);
        Ok(Clicks::new(read("d"), read("c"), read("m"), read("y"), read("w")))
    }
}

/// The Scanner face's machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Machine {
    #[default]
    Oni,
    Frontier,
}

/// The Colour tab's finishing on the Scanner face, kept apart from Pro's.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct Finish {
    pub contrast: f32,
    #[serde(rename = "manualCurves")]
    pub manual_curves: CurveSet,
    pub spectrogram: Spectrogram,
    pub saturation: SaturationCurve,
    pub vignette: Vignette,
    #[serde(rename = "secretSauce")]
    pub secret_sauce: f32,
}

/// The Scanner face's fields as a settings object of format 45 and under names them.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct Scanner {
    pub face: Face,
    #[serde(rename = "scanner")]
    pub clicks: Clicks,
    #[serde(rename = "scannerMachine")]
    pub machine: Machine,
    #[serde(rename = "scannerFinish")]
    pub finish: Finish,
}

impl Scanner {
    /// The fields `settings` carries, at rest where it names none; a value of the wrong shape refuses the document, as
    /// its typed read did.
    pub fn read(settings: Option<&Value>) -> Result<Scanner, Failure> {
        match settings {
            Some(v @ Value::Object(_)) => serde_json::from_value(v.clone()).map_err(|e| Failure::Malformed(e.to_string())),
            _ => Ok(Scanner::default()),
        }
    }
}

/// The Lighting exposure, in format 46's units, the Scanner face's machine printed at: its looks lend about a stop for a
/// negative's 1.5 of rest left out of the curve, so a slide, resting at 0, entered its curve as far under its own rest.
pub fn look_entry_exposure(mode: ConversionMode) -> f32 {
    former_rest(mode) - FORMER_REST / GEAR
}

/// `settings` of `version` on the one face: the edit of the face that rendered it, every step before this one done.
pub fn caught_up(version: i64, settings: PipelineSettings, scanner: &Scanner) -> PipelineSettings {
    // A digital photo kept on the Scanner face before format 30 rendered as Pro.
    let face = if version < DIGITAL_SCANNER_VERSION && settings.mode.is_digital() { Face::Pro } else { scanner.face };
    if version >= ONE_FACE_VERSION || face == Face::Pro {
        return settings;
    }
    printed_on_the_machine(settings, scanner)
}

/// What the Scanner face rendered, in the one face's fields: its finishing, Lighting, Colour Density and Auto White Balance
/// at the machine's rest; the keys and the look a legacy print, none on a paper whose keys rest, where it lays nothing.
fn printed_on_the_machine(settings: PipelineSettings, scanner: &Scanner) -> PipelineSettings {
    let (f, k) = (&scanner.finish, scanner.clicks);
    let mut out = settings;
    out.manual_curves = f.manual_curves.clone();
    out.spectrogram = f.spectrogram;
    out.saturation = f.saturation.clone();
    out.vignette = f.vignette;
    out.secret_sauce = f.secret_sauce;
    let exposure = if super::chain::papered(&out) { former_rest(out.mode) } else { look_entry_exposure(out.mode) };
    let held = out.print;
    out.print = Print { contrast: f.contrast, exposure, colour_compensation: held.colour_compensation, print_lift: held.print_lift, ..Print::default() };
    out.density = 0.0;
    out.balance_filters = None;
    let machine = match scanner.machine {
        Machine::Oni => LegacyMachine::Oni,
        Machine::Frontier => LegacyMachine::Frontier,
    };
    let laid = LegacyPrint { machine, d: k.d, c: k.c, m: k.m, y: k.y, w: k.w, laid: true };
    out.legacy_print = (!super::chain::papered(&out) || !k.is_neutral()).then_some(laid);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PaperStock;
    use serde_json::json;

    #[test]
    fn a_keypad_reads_absent_keys_at_rest_and_clamps_past_its_stops() {
        let absent: Clicks = serde_json::from_str("{}").unwrap();
        assert_eq!(absent, Clicks::default());
        let past: Clicks = serde_json::from_str(r#"{"d":50,"c":-60,"w":21}"#).unwrap();
        assert_eq!(past, Clicks::new(48, -48, 0, 0, 20));
        // Twin: counts on and inside the stops read as written, in steps, an unknown key ignored.
        let inside: Clicks = serde_json::from_str(r#"{"d":-48,"m":7,"y":-3,"w":-20,"s":20}"#).unwrap();
        assert_eq!((inside.d, inside.m, inside.y, inside.w), (-48, 7, -3, -20));
    }

    fn scanner(face: &str, machine: &str) -> Scanner {
        let stored = json!({
            "face": face,
            "scanner": {"d": 3, "c": -1, "m": 0, "y": 2, "w": 1},
            "scannerMachine": machine,
            "scannerFinish": {"contrast": 0.2, "secretSauce": 0.4, "vignette": {"stops": -0.5}},
        });
        Scanner::read(Some(&stored)).unwrap()
    }

    fn pro_edit() -> PipelineSettings {
        let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
        s.print.exposure = 2.0;
        s.print.shadows = 0.1;
        s.print.red.exposure = 0.3;
        s.print.colour_compensation = false;
        s.density = 0.25;
        s.balance_filters = Some([0.1, 0.0, -0.1]);
        s.secret_sauce = 0.9;
        s
    }

    #[test]
    fn a_scanner_photo_keeps_its_machines_print_its_finishing_and_its_keys() {
        let older = ONE_FACE_VERSION - 1;
        let s = caught_up(older, pro_edit(), &scanner("scanner", "frontier"));
        let print = Print { contrast: 0.2, exposure: look_entry_exposure(ConversionMode::Negative), colour_compensation: false, ..Print::default() };
        assert_eq!(s.print, print);
        assert_eq!((s.density, s.balance_filters, s.secret_sauce, s.vignette.stops), (0.0, None, 0.4, -0.5));
        assert_eq!(s.legacy_print, Some(LegacyPrint { machine: LegacyMachine::Frontier, d: 3, c: -1, m: 0, y: 2, w: 1, laid: true }));
        // On a paper its print rests at the paper's rest, and keys at rest lay no legacy print.
        let papered = PipelineSettings { print_paper: Some(PaperStock::default()), ..pro_edit() };
        let mut bare = scanner("scanner", "oni");
        bare.clicks = Clicks::default();
        let on_paper = caught_up(older, papered.clone(), &bare);
        assert_eq!((on_paper.print.exposure, on_paper.legacy_print), (FORMER_REST, None));
        assert!(caught_up(older, papered, &scanner("scanner", "oni")).legacy_print.is_some(), "twin: its keys moved, it lays them");
    }

    #[test]
    fn a_pro_photo_and_an_early_digital_one_keep_pros_edit_whole() {
        let older = ONE_FACE_VERSION - 1;
        assert_eq!(caught_up(older, pro_edit(), &scanner("pro", "frontier")), pro_edit());
        let digital = PipelineSettings { mode: ConversionMode::Digital, ..pro_edit() };
        assert_eq!(caught_up(DIGITAL_SCANNER_VERSION - 1, digital.clone(), &scanner("scanner", "oni")), digital);
        // Twin: the same digital photo stored at 30 rendered on the machine, and a document of this format moves nothing.
        assert_ne!(caught_up(DIGITAL_SCANNER_VERSION, digital.clone(), &scanner("scanner", "oni")), digital);
        assert_eq!(caught_up(ONE_FACE_VERSION, pro_edit(), &scanner("scanner", "oni")), pro_edit());
    }

    #[test]
    fn a_face_of_the_wrong_shape_refuses_the_document_as_its_typed_read_did() {
        assert!(Scanner::read(Some(&json!({"face": "lab"}))).is_err());
        assert_eq!(Scanner::read(Some(&json!({}))).unwrap(), Scanner::default());
        assert_eq!(Scanner::read(None).unwrap(), Scanner::default());
    }
}
