//! A paper's colour compression in the sidecar: written only off its rest, read back, an older document's absent value
//! read as the paper as baked, a stored value held to the widest travel, and a document carrying it newer than any build
//! that cannot read it. Each verdict has a twin.

use oni_settings::document::VERSION;
use oni_settings::migrate::{self, chain};
use oni_settings::{ConversionMode, Document, PaperStock, PipelineSettings};
use serde_json::{Value, json};

fn stored(settings: &PipelineSettings) -> Value {
    migrate::reshape::as_stored(&Document::of(settings.clone()), VERSION)
}

fn read(document: &Value) -> PipelineSettings {
    migrate::document(&serde_json::to_vec(document).unwrap()).unwrap().document.settings
}

fn compressed(k: f32) -> PipelineSettings {
    let mut s = PipelineSettings { print_paper: Some(PaperStock::of("kodak-endura-premier")), ..PipelineSettings::rest(ConversionMode::Negative, true) };
    s.print.colour_compression = k;
    s
}

#[test]
fn colour_compression_is_written_only_off_its_rest_and_read_back() {
    let print = &stored(&compressed(0.4))["settings"]["print"];
    assert_eq!(print["colourCompression"], json!(0.4f32));
    assert_eq!(read(&stored(&compressed(0.4))), compressed(0.4));
    // Twin: at rest the key is left out, so a document at rest is written as before.
    assert!(stored(&compressed(1.0))["settings"]["print"].get("colourCompression").is_none());
}

#[test]
fn an_older_document_reads_the_paper_as_baked_and_a_value_past_the_travel_lands_on_its_stop() {
    let mut older = stored(&compressed(1.0));
    older["version"] = json!(chain::COLOUR_COMPRESSION_VERSION - 1);
    assert_eq!(read(&older), compressed(1.0), "format 84: the paper as it printed");
    let mut wide = stored(&compressed(1.0));
    wide["settings"]["print"]["colourCompression"] = json!(7.0);
    assert_eq!(read(&wide).print.colour_compression, oni_settings::tone::Print::COMPRESSION_RANGE.1);
    // Twin: a value within the travel is kept as stored.
    wide["settings"]["print"]["colourCompression"] = json!(0.25);
    assert_eq!(read(&wide).print.colour_compression, 0.25);
}

#[test]
fn a_document_that_carries_it_is_newer_than_any_build_that_cannot_read_it() {
    let written = stored(&compressed(0.4));
    assert!(written["version"].as_i64().unwrap() >= chain::COLOUR_COMPRESSION_VERSION && chain::COLOUR_COMPRESSION_VERSION > chain::PAPERS_REDONE_VERSION);
    let mut ahead = written.clone();
    ahead["version"] = json!(VERSION + 1);
    assert_eq!(migrate::document(&serde_json::to_vec(&ahead).unwrap()), Err(migrate::Refusal::Newer(VERSION + 1)));
    // Twin: today's reader reads its own version.
    assert_eq!(read(&written), compressed(0.4));
}

#[test]
fn every_shipped_paper_bakes_a_reach_that_holds_the_rest() {
    for p in PaperStock::all() {
        let limit = p.compression_limit().unwrap_or_else(|| panic!("{p:?}: a shipped paper bakes its reach"));
        assert!((1.0..=oni_settings::tone::Print::COMPRESSION_RANGE.1).contains(&limit), "{p:?}: {limit}");
    }
    // Twin: a key no file defines bakes none.
    assert_eq!(PaperStock::of("oni-standard").compression_limit(), None);
}

#[test]
fn a_value_past_a_papers_reach_prints_and_shows_that_reach_and_is_kept_for_a_paper_reaching_it() {
    let mut s = compressed(1.4);
    s.print_paper = Some(PaperStock::of("fujifilm-crystal-archive-digital-dpii"));
    assert_eq!(s.colour_compression(), 1.4);
    // Royal Digital reaches 1.2: the print and the slider read 1.2, the stored 1.4 stays.
    let royal = PipelineSettings { print_paper: Some(PaperStock::of("kodak-royal-digital")), ..s.clone() };
    assert_eq!((royal.colour_compression(), royal.print.colour_compression), (1.2, 1.4));
    assert_eq!(read(&stored(&royal)).print.colour_compression, 1.4);
    // Twin: no paper prints none, and back on Crystal Archive the stored 1.4 prints again.
    assert_eq!(PipelineSettings { print_paper: None, ..s.clone() }.colour_compression(), 1.0);
    assert_eq!(PipelineSettings { print_paper: royal.print_paper, ..royal.clone() }.colour_compression(), 1.2);
    assert_eq!(PipelineSettings { print_paper: s.print_paper, ..royal }.colour_compression(), 1.4);
}
