//! Sidecar format 46: the Scanner face gone, a photo it rendered keeps that edit in the one face's fields and a legacy
//! print, a photo Pro rendered keeps Pro's whole, and a digital photo kept there before format 30 opens on Pro.

use oni_settings::document::{self, VERSION};
use oni_settings::migrate::{self, Facts, Refusal, chain};
use oni_settings::{ConversionMode, Document, LegacyMachine, LegacyPrint, PaperStock, PipelineSettings};
use serde_json::{Value, json};

/// `settings` written at `version`, the Scanner face's fields laid in as that build stored them.
fn stored(settings: &PipelineSettings, version: i64, face: &str, machine: &str) -> Vec<u8> {
    let mut object = serde_json::to_value(Document::of(settings.clone())).unwrap();
    object["version"] = json!(version);
    let s = &mut object["settings"];
    s["face"] = json!(face);
    s["scannerMachine"] = json!(machine);
    s["scanner"] = json!({"d": 3, "c": 0, "m": -2, "y": 1, "w": 4});
    s["scannerFinish"] = json!({"contrast": 0.15, "secretSauce": 0.3});
    serde_json::to_vec(&object).unwrap()
}

/// The settings a sidecar's read and a document's read both give, which agree.
fn read(bytes: &[u8]) -> PipelineSettings {
    let (sidecar, held) = migrate::reading(bytes, &Facts::bare(true), false).unwrap();
    assert!(held.is_none());
    let document = migrate::document(bytes).unwrap().document.settings;
    assert_eq!(sidecar.document.settings, document);
    document
}

fn edited(mode: ConversionMode) -> PipelineSettings {
    let mut s = PipelineSettings::rest(mode, true);
    s.print.exposure = 1.8;
    s.print.highlights = -0.2;
    s.print.blue.exposure = 0.2;
    s.density = 0.3;
    s.secret_sauce = 0.7;
    s
}

#[test]
fn a_scanner_photo_reads_its_machines_print_and_its_keys_in_a_legacy_print() {
    let before = chain::FRONTIER_PRINT_VERSION;
    let s = read(&stored(&edited(ConversionMode::Negative), before, "scanner", "frontier"));
    let print = s.print;
    // The machine's entry is the legacy print's own: Lighting's Exposure rests.
    assert_eq!((print.exposure, print.contrast, print.highlights, print.filters()), (0.0, 0.15, 0.0, [0.0; 3]));
    assert_eq!((s.density, s.secret_sauce), (0.0, 0.3));
    assert_eq!(s.legacy_print, Some(LegacyPrint { machine: LegacyMachine::Frontier, d: 3, c: 0, m: -2, y: 1, w: 4, laid: true }));
    // On a paper with every key at rest it prints the paper's print alone: no legacy print.
    let edge = PipelineSettings { print_paper: Some(PaperStock::of("kodak-ektacolor-edge")), ..edited(ConversionMode::Negative) };
    let mut papered = serde_json::from_slice::<Value>(&stored(&edge, before, "scanner", "oni")).unwrap();
    papered["settings"]["scanner"] = json!({});
    let on_paper = read(&serde_json::to_vec(&papered).unwrap());
    assert_eq!((on_paper.legacy_print, on_paper.print.exposure, on_paper.print_paper), (None, 0.0, Some(PaperStock::of("kodak-ektacolor-edge"))));
}

#[test]
fn a_pro_photo_keeps_pros_edit_whole_and_its_scanner_fields_go() {
    let before = chain::FRONTIER_PRINT_VERSION;
    let pro = read(&stored(&edited(ConversionMode::Negative), before, "pro", "frontier"));
    let mut bare = serde_json::to_value(Document::of(edited(ConversionMode::Negative))).unwrap();
    bare["version"] = json!(before);
    assert_eq!(pro, read(&serde_json::to_vec(&bare).unwrap()), "as if it never named the Scanner");
    assert_eq!(pro.legacy_print, None);
    // Twin: the same photo stored on the Scanner face takes the Scanner's edit, not Pro's.
    assert_ne!(read(&stored(&edited(ConversionMode::Negative), before, "scanner", "frontier")), pro);
}

#[test]
fn a_digital_photo_stored_on_the_scanner_before_the_machine_opens_on_pro() {
    let before = chain::DIGITAL_SCANNER_VERSION - 1;
    let rest = PipelineSettings::rest(ConversionMode::Digital, true);
    let mut then = rest.clone();
    (then.print.exposure, then.grain.amount) = (0.0, 0.0);
    let s = read(&stored(&then, before, "scanner", "oni"));
    // Its other values are its rest, and an earlier photo stays on ONI's print with no grain.
    assert_eq!(s, PipelineSettings { print_paper: None, grain: oni_settings::blocks::Grain { amount: 0.0, ..rest.grain }, ..rest });
    // Twin: a negative of that format and a digital photo stored at 30 printed on the machine.
    assert!(read(&stored(&PipelineSettings::rest(ConversionMode::Negative, true), before, "scanner", "oni")).legacy_print.is_some());
    assert!(read(&stored(&then, chain::DIGITAL_SCANNER_VERSION, "scanner", "oni")).legacy_print.is_some());
}

#[test]
fn format_46_is_written_and_read_back_and_a_build_of_45_refuses_it_as_newer() {
    assert!(VERSION >= chain::ONE_FACE_VERSION && chain::ONE_FACE_VERSION == chain::FRONTIER_PRINT_VERSION + 1);
    let mut s = edited(ConversionMode::Negative);
    s.legacy_print = Some(LegacyPrint { machine: LegacyMachine::Oni, d: -1, c: 2, m: 0, y: 0, w: 0, laid: false });
    let text = document::encode(&Document::of(s.clone())).unwrap();
    let written: Value = serde_json::from_slice(&text).unwrap();
    assert_eq!(written["settings"]["legacyPrint"], json!({"machine": "oni", "d": -1, "c": 2, "m": 0, "y": 0, "w": 0, "laid": false}));
    assert!(written["settings"].get("face").is_none() && written["settings"].get("scanner").is_none(), "the Scanner's keys are written no more");
    assert_eq!(document::decode(&text).unwrap().settings, s);
    assert_eq!(read(&text), s, "a document of this format moves nothing");
    // A reader refuses a version past its own, as a build of 45 refuses one of 46.
    let mut newer = written.clone();
    newer["version"] = json!(VERSION + 1);
    assert_eq!(migrate::document(&serde_json::to_vec(&newer).unwrap()).err(), Some(Refusal::Newer(VERSION + 1)));
    // Twin: a reader of this version reads its own.
    assert!(migrate::document(&text).is_ok());
}

/// A folder of sidecars, read in memory and never written: how many photos the Scanner face rendered and took a legacy
/// print, by machine, and that every Pro photo kept Pro's edit; `ONI_ONE_FACE_DIR` names it, the corpus without it.
#[test]
fn a_folder_of_sidecars_reads_on_the_one_face_and_counts_its_legacy_prints() {
    let dir = std::env::var_os("ONI_ONE_FACE_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus"));
    let mut stack = vec![dir.clone()];
    let (mut read_back, mut scanner, mut legacy, mut on_paper, mut frontier, mut refused, mut bare) = (0, 0, 0, 0, 0, 0, 0);
    while let Some(d) = stack.pop() {
        for p in std::fs::read_dir(&d).unwrap().flatten().map(|e| e.path()) {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if !name.ends_with(".json") || name.ends_with(".facts.json") || name.starts_with("._") {
                continue;
            }
            let bytes = std::fs::read(&p).unwrap();
            let Ok(stored) = serde_json::from_slice::<Value>(&bytes) else { continue };
            let Ok(converted) = migrate::document(&bytes) else {
                refused += 1;
                continue;
            };
            read_back += 1;
            let s = converted.document.settings;
            if stored["settings"]["face"] == "scanner" {
                scanner += 1;
                legacy += usize::from(s.legacy_print.is_some());
                on_paper += usize::from(s.paper().is_some());
                frontier += usize::from(s.legacy_print.is_some_and(|l| l.machine == LegacyMachine::Frontier));
                bare += usize::from(s.paper().is_none() && s.legacy_print.is_none());
            } else {
                assert_eq!(s.legacy_print, None, "{name}: a Pro photo takes no legacy print");
            }
        }
    }
    println!("{}: {read_back} read, {refused} refused, {scanner} rendered by the Scanner face, {legacy} on a legacy print ({frontier} Frontier), {on_paper} on a paper", dir.display());
    assert_eq!(bare, 0, "every photo the Scanner face rendered off a paper takes a legacy print");
    // Twin: the folder holds such photos, so the count above saw some.
    assert!(scanner > 0 && legacy > 0, "{scanner} {legacy}");
}
