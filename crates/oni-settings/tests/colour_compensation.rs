//! Lighting's colour compensation in the sidecar: written only when off, read on from every older document, and a
//! document that carries it stamped past every build that cannot read it. Each verdict has a twin.

use oni_settings::document::VERSION;
use oni_settings::migrate::{self, chain};
use oni_settings::{ConversionMode, Document, PipelineSettings};
use serde_json::{Value, json};

fn stored(settings: &PipelineSettings) -> Value {
    migrate::reshape::as_stored(&Document::of(settings.clone()), VERSION)
}

fn off() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.print.colour_compensation = false;
    s
}

fn read(document: &Value) -> PipelineSettings {
    migrate::document(&serde_json::to_vec(document).unwrap()).unwrap().document.settings
}

#[test]
fn the_switch_is_written_only_when_off_and_read_back() {
    let (on, off) = (PipelineSettings::rest(ConversionMode::Negative, true), off());
    assert_eq!(stored(&off)["settings"]["print"]["colourCompensation"], json!(false));
    assert_eq!(read(&stored(&off)), off);
    // Twin: on, the key is left out and the document reads on.
    assert!(stored(&on)["settings"]["print"].get("colourCompensation").is_none());
    assert!(read(&stored(&on)).print.colour_compensation);
}

#[test]
fn every_older_document_reads_it_on() {
    for version in [chain::NAMED_VERSION, chain::PAPER_VERSION, chain::EXPOSURE_GEAR_VERSION] {
        let mut older = stored(&PipelineSettings::rest(ConversionMode::Negative, true));
        older["version"] = json!(version);
        assert!(read(&older).print.colour_compensation, "format {version}");
        // Twin: the same document carrying the key off reads it off, so the reader sees the key.
        older["settings"]["print"]["colourCompensation"] = json!(false);
        assert!(!read(&older).print.colour_compensation, "format {version}");
    }
}

#[test]
fn a_document_that_carries_it_is_newer_than_any_build_that_cannot_read_it() {
    let written = stored(&off());
    assert_eq!(written["version"], json!(chain::COLOUR_COMPENSATION_VERSION));
    assert_eq!(chain::COLOUR_COMPENSATION_VERSION, chain::EXPOSURE_GEAR_VERSION + 1);
    // A reader refuses any version past its own, as a build of 42 refuses this one.
    let mut ahead = written.clone();
    ahead["version"] = json!(VERSION + 1);
    assert_eq!(migrate::document(&serde_json::to_vec(&ahead).unwrap()), Err(migrate::Refusal::Newer(VERSION + 1)));
    // Twin: today's reader reads its own version.
    assert_eq!(read(&written), off());
}
