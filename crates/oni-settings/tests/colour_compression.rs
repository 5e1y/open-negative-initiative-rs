//! A paper's colour compression in the sidecar, the share of chroma its paper takes: none its own dose and written only
//! once moved, a picked paper at its own dose as a picked film at its effects', an older document's strength read as its
//! paper's share, and a document carrying it newer than any build that cannot read it. Each verdict has a twin.

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

const ENDURA: &str = "kodak-endura-premier";

fn on(paper: &'static str, share: Option<f32>) -> PipelineSettings {
    let mut s = PipelineSettings { print_paper: Some(PaperStock::of(paper)), ..PipelineSettings::rest(ConversionMode::Negative, true) };
    s.print.colour_compression = share;
    s
}

#[test]
fn a_share_is_written_once_moved_and_read_back_and_none_is_the_papers_own_dose() {
    assert_eq!(stored(&on(ENDURA, Some(0.4)))["settings"]["print"]["colourCompression"], json!(0.4f32));
    assert_eq!(read(&stored(&on(ENDURA, Some(0.4)))), on(ENDURA, Some(0.4)));
    let scale = PaperStock::of(ENDURA).compression().unwrap();
    assert_eq!((on(ENDURA, None).colour_compression(), on(ENDURA, None).compression_strength()), (scale.rest(), 1.0));
    assert_eq!(on(ENDURA, Some(scale.rest())).compression_strength(), 1.0, "its own dose stored is its own strength, exactly");
    // Twin: at its own dose the key is left out.
    assert!(stored(&on(ENDURA, None))["settings"]["print"].get("colourCompression").is_none());
}

#[test]
fn a_picked_paper_takes_its_own_dose_and_the_paper_in_use_picked_again_writes_nothing() {
    let moved = on(ENDURA, Some(0.4));
    let royal = moved.on_paper(Some(PaperStock::of("kodak-royal-digital")));
    assert_eq!((royal.print.colour_compression, royal.colour_compression()), (None, PaperStock::of("kodak-royal-digital").compression().unwrap().rest()));
    // Twin: the paper in use picked again keeps the hand's share.
    assert_eq!(moved.on_paper(Some(PaperStock::of(ENDURA))), moved);
}

#[test]
fn an_older_documents_strength_is_read_as_its_papers_share_and_a_share_past_the_travel_lands_on_its_stop() {
    let scale = PaperStock::of(ENDURA).compression().unwrap();
    let mut older = stored(&on(ENDURA, None));
    older["version"] = json!(chain::COLOUR_COMPRESSION_VERSION);
    older["settings"]["print"]["colourCompression"] = json!(0.5);
    let read_back = read(&older);
    assert_eq!(read_back.print.colour_compression, Some(scale.removed_at(0.5)));
    assert!((read_back.compression_strength() - 0.5).abs() < 1e-5, "the same strength prints");
    let mut before = stored(&on(ENDURA, None));
    before["version"] = json!(chain::PAPERS_REDONE_VERSION);
    assert_eq!(read(&before), on(ENDURA, None), "before format 85, the paper as baked");
    let mut wide = stored(&on(ENDURA, None));
    wide["settings"]["print"]["colourCompression"] = json!(7.0);
    assert_eq!(read(&wide).print.colour_compression, Some(oni_settings::tone::Print::COMPRESSION_RANGE.1));
    // Twin: a share within the travel is kept as stored.
    wide["settings"]["print"]["colourCompression"] = json!(0.25);
    assert_eq!(read(&wide).print.colour_compression, Some(0.25));
}

#[test]
fn a_document_that_carries_it_is_newer_than_any_build_that_cannot_read_it() {
    let written = stored(&on(ENDURA, Some(0.4)));
    assert!(written["version"].as_i64().unwrap() >= chain::ABSOLUTE_COMPRESSION_VERSION && chain::ABSOLUTE_COMPRESSION_VERSION > chain::COLOUR_COMPRESSION_VERSION);
    let mut ahead = written.clone();
    ahead["version"] = json!(VERSION + 1);
    assert_eq!(migrate::document(&serde_json::to_vec(&ahead).unwrap()), Err(migrate::Refusal::Newer(VERSION + 1)));
    // Twin: today's reader reads its own version.
    assert_eq!(read(&written), on(ENDURA, Some(0.4)));
}

#[test]
fn every_shipped_paper_bakes_a_scale_from_the_pure_dyes_through_its_own_dose() {
    for p in PaperStock::all() {
        let c = p.compression().unwrap_or_else(|| panic!("{p:?}: a shipped paper bakes its scale"));
        assert!(c.rest() > 0.0 && c.rest() < c.reach() && c.reach() <= oni_settings::tone::Print::COMPRESSION_RANGE.1 + 0.05, "{p:?}");
        println!("{}: {:.1} % at its own dose, {:.1} % at its reach, strength {}", p.name(), 100.0 * c.rest(), 100.0 * c.reach(), c.strengths.last().unwrap());
    }
    // Twin: a key no file defines bakes none, and a scale that does not rise is refused.
    assert!(PaperStock::of("oni-standard").compression().is_none());
    assert!(oni_settings::paper::CompressionScale::new(vec![0.0, 1.0], vec![0.0, 0.0]).is_none());
}

#[test]
fn a_share_past_a_papers_reach_prints_and_shows_that_reach_and_is_kept() {
    let s = on("fujifilm-crystal-archive-digital-dpii", Some(0.5));
    assert_eq!(s.colour_compression(), 0.5);
    let edge = PipelineSettings { print_paper: Some(PaperStock::of("kodak-ektacolor-edge")), ..s.clone() };
    let reach = PaperStock::of("kodak-ektacolor-edge").compression().unwrap().reach();
    assert_eq!((edge.colour_compression(), edge.print.colour_compression), (reach, Some(0.5)));
    // Twin: no paper takes none, and back on Crystal Archive the stored share prints again.
    assert_eq!(PipelineSettings { print_paper: None, ..s.clone() }.colour_compression(), 0.0);
    assert_eq!(PipelineSettings { print_paper: s.print_paper, ..edge }.colour_compression(), 0.5);
}
