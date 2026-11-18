//! A digital photo stored on the Scanner face before the machine printed one opens on Pro, the face it
//! rendered on; a negative keeps its Scanner face, and a file of this format keeps whatever it names.

use oni_settings::document::VERSION;
use oni_settings::migrate::{self, Facts, chain};
use oni_settings::{ConversionMode, Document, Face, PipelineSettings};
use serde_json::json;

/// `settings` written at `version`.
fn stored(settings: &PipelineSettings, version: i64) -> Vec<u8> {
    let mut object = serde_json::to_value(Document::of(settings.clone())).unwrap();
    object["version"] = json!(version);
    serde_json::to_vec(&object).unwrap()
}

fn on_scanner(mode: ConversionMode) -> PipelineSettings {
    PipelineSettings { face: Face::Scanner, ..PipelineSettings::rest(mode, true) }
}

/// The face a sidecar's read and a document's read both give.
fn faces(bytes: &[u8]) -> (Face, Face) {
    let (read, held) = migrate::reading(bytes, &Facts::bare(true), false).unwrap();
    assert!(held.is_none());
    (read.document.settings.face, migrate::document(bytes).unwrap().document.settings.face)
}

#[test]
fn a_digital_photo_stored_on_the_scanner_before_the_machine_opens_on_pro() {
    let before = chain::DIGITAL_SCANNER_VERSION - 1;
    assert!(chain::DIGITAL_SCANNER_VERSION <= VERSION);
    assert_eq!(faces(&stored(&on_scanner(ConversionMode::Digital), before)), (Face::Pro, Face::Pro));
    // Its other values are as stored.
    let (read, _) = migrate::reading(&stored(&on_scanner(ConversionMode::Digital), before), &Facts::bare(true), false).unwrap();
    assert_eq!(read.document.settings, PipelineSettings::rest(ConversionMode::Digital, true));
}

#[test]
fn twin_a_negative_before_it_and_a_digital_photo_of_this_format_keep_the_scanner() {
    let before = chain::DIGITAL_SCANNER_VERSION - 1;
    assert_eq!(faces(&stored(&on_scanner(ConversionMode::Negative), before)), (Face::Scanner, Face::Scanner));
    assert_eq!(faces(&stored(&on_scanner(ConversionMode::Digital), VERSION)), (Face::Scanner, Face::Scanner));
}
