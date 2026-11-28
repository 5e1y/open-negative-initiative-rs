//! A paper's Brightness and Print lift in the sidecar: written only when moved, an older document's retired switches
//! dropped, carried by a settings paste, and a document that carries them stamped past every build that cannot read them.
//! Each verdict has a twin.

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
    s.print_paper = Some(PaperStock::of("kodak-endura-premier"));
    (s.print.brightness, s.print.print_lift) = (0.75, true);
    s
}

fn read(document: &Value) -> PipelineSettings {
    migrate::document(&serde_json::to_vec(document).unwrap()).unwrap().document.settings
}

#[test]
fn brightness_and_print_lift_are_written_only_when_moved_and_read_back() {
    let print = &stored(&lit())["settings"]["print"];
    assert_eq!((print["brightness"].clone(), print["printLift"].clone()), (json!(0.75), json!(true)));
    assert!(print.get("trueBlacks").is_none() && print.get("paperBlacks").is_none(), "the retired keys are never written");
    assert_eq!(read(&stored(&lit())), lit());
    // Twin: at rest both keys are left out, so a document at rest is written as before.
    let rest = PipelineSettings::rest(ConversionMode::Negative, true);
    let print = &stored(&rest)["settings"]["print"];
    assert!(print.get("brightness").is_none() && print.get("printLift").is_none());
}

#[test]
fn a_new_photo_names_no_paper_and_print_lift_rests_off() {
    let digital = PipelineSettings::rest(ConversionMode::Digital, true);
    assert!(digital.paper().is_none() && digital.shows_inversion() && !digital.print.print_lift, "a new photo on no paper, Print lift off");
    // Twin: the switch, once on, is what a sidecar keeps.
    let on = PipelineSettings { print: oni_settings::tone::Print { print_lift: true, ..digital.print }, ..digital.clone() };
    assert!(read(&stored(&on)).print.print_lift);
}

#[test]
fn an_older_document_opens_its_retired_switches_dropped_and_its_paper_kept_only_where_chosen() {
    for version in [chain::PAPER_ENDS_VERSION, chain::PAPER_BLACKS_VERSION, chain::PRINT_LIFT_VERSION - 1] {
        let mut older = stored(&PipelineSettings { print_paper: Some(PaperStock::of("kodak-endura-premier")), ..PipelineSettings::rest(ConversionMode::Digital, true) });
        older["version"] = json!(version);
        (older["settings"]["print"]["trueBlacks"], older["settings"]["print"]["paperBlacks"]) = (json!(true), json!(true));
        older["settings"]["print"]["brightness"] = json!(-0.5);
        let s = read(&older);
        assert!(s.print.brightness == -0.5 && !s.print.print_lift && s.print_paper.is_none(), "format {version}: a default paper then");
    }
    // Twin: from format 73 a paper is a choice, and it stays.
    let mut chosen = stored(&lit());
    chosen["version"] = json!(chain::OPTIONAL_PAPER_VERSION);
    assert_eq!(read(&chosen), lit());
}

#[test]
fn a_paste_carries_them_with_lighting() {
    let target = PipelineSettings::rest(ConversionMode::Negative, true);
    let pasted = look_settings::copied(&lit(), &target);
    assert!(pasted.print.brightness == 0.75 && pasted.print.print_lift && pasted.print_paper.is_some());
    // Twin: the target's own Lighting before the paste.
    assert!(target.print.brightness == 0.0 && !target.print.print_lift);
}

#[test]
fn a_document_that_carries_them_is_newer_than_any_build_that_cannot_read_them() {
    let written = stored(&lit());
    assert!(written["version"].as_i64().unwrap() >= chain::PRINT_LIFT_VERSION);
    assert!(chain::PRINT_LIFT_VERSION > chain::UNMIXED_VERSION && chain::PAPER_BLACKS_VERSION > chain::ONE_FACE_VERSION);
    // A reader refuses any version past its own, as a build of 46 refuses this one.
    let mut ahead = written.clone();
    ahead["version"] = json!(VERSION + 1);
    assert_eq!(migrate::document(&serde_json::to_vec(&ahead).unwrap()), Err(migrate::Refusal::Newer(VERSION + 1)));
    // Twin: today's reader reads its own version.
    assert_eq!(read(&written), lit());
}
