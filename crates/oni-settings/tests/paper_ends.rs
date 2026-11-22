//! A paper's Brightness and True blacks in the sidecar: written only when moved, read at rest from every older document,
//! carried by a settings paste, and a document that carries them stamped past every build that cannot read them. Each
//! verdict has a twin.

use oni_settings::document::VERSION;
use oni_settings::migrate::{self, chain};
use oni_settings::settings::look_settings;
use oni_settings::{ConversionMode, Document, PaperStock, PipelineSettings};
use serde_json::{Value, json};

fn stored(settings: &PipelineSettings) -> Value {
    migrate::reshape::as_stored(&Document::of(settings.clone()), VERSION)
}

fn lit() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.print_paper = Some(PaperStock::default());
    (s.print.brightness, s.print.true_blacks) = (0.75, true);
    s
}

fn read(document: &Value) -> PipelineSettings {
    migrate::document(&serde_json::to_vec(document).unwrap()).unwrap().document.settings
}

#[test]
fn brightness_and_true_blacks_are_written_only_when_moved_and_read_back() {
    let print = &stored(&lit())["settings"]["print"];
    assert_eq!((print["brightness"].clone(), print["trueBlacks"].clone()), (json!(0.75), json!(true)));
    assert_eq!(read(&stored(&lit())), lit());
    // Twin: at rest both keys are left out, so a document at rest is written as before.
    let rest = PipelineSettings::rest(ConversionMode::Negative, true);
    let print = &stored(&rest)["settings"]["print"];
    assert!(print.get("brightness").is_none() && print.get("trueBlacks").is_none());
}

#[test]
fn every_older_document_reads_them_at_rest() {
    for version in [chain::NAMED_VERSION, chain::PAPER_VERSION, chain::COLOUR_COMPENSATION_VERSION] {
        let mut older = stored(&PipelineSettings { print_paper: Some(PaperStock::default()), ..PipelineSettings::rest(ConversionMode::Digital, true) });
        older["version"] = json!(version);
        let s = read(&older);
        assert!(s.print.brightness == 0.0 && !s.print.true_blacks, "format {version}");
        // Twin: the same document carrying the keys reads them, so the reader sees the keys.
        older["settings"]["print"]["brightness"] = json!(-0.5);
        older["settings"]["print"]["trueBlacks"] = json!(true);
        let s = read(&older);
        assert!(s.print.brightness == -0.5 && s.print.true_blacks, "format {version}");
    }
}

#[test]
fn a_paste_carries_them_with_lighting() {
    let target = PipelineSettings::rest(ConversionMode::Negative, true);
    let pasted = look_settings::copied(&lit(), &target);
    assert!(pasted.print.brightness == 0.75 && pasted.print.true_blacks && pasted.print_paper.is_some());
    // Twin: the target's own Lighting before the paste.
    assert!(target.print.brightness == 0.0 && !target.print.true_blacks);
}

#[test]
fn a_document_that_carries_them_is_newer_than_any_build_that_cannot_read_them() {
    let written = stored(&lit());
    assert!(written["version"].as_i64().unwrap() >= chain::PAPER_ENDS_VERSION);
    assert_eq!(chain::PAPER_ENDS_VERSION, chain::COLOUR_COMPENSATION_VERSION + 1);
    // A reader refuses any version past its own, as a build of 43 refuses this one.
    let mut ahead = written.clone();
    ahead["version"] = json!(VERSION + 1);
    assert_eq!(migrate::document(&serde_json::to_vec(&ahead).unwrap()), Err(migrate::Refusal::Newer(VERSION + 1)));
    // Twin: today's reader reads its own version.
    assert_eq!(read(&written), lit());
}
