//! A digital photo's virtual film in a sidecar: absent before its format and read as none, a film kept to
//! the bit, none written as nothing, a build before it refusing to rewrite the file; each verdict with a twin.

use oni_settings::document::VERSION;
use oni_settings::migrate::{self, Facts, Refusal, chain};
use oni_settings::{ConversionMode, Document, PipelineSettings, VirtualFilm, VirtualStock};
use serde_json::json;

fn filmed() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
    let stock = VirtualStock::KodakPortra800;
    s.virtual_film = Some(VirtualFilm { stock, exposure: 0.7, stops: [-0.8, 0.0, 1.2], black: [0.27; 3], white: [0.54, 0.6, 0.62] });
    s
}

fn at(settings: &PipelineSettings, version: i64) -> Vec<u8> {
    let mut object = serde_json::to_value(Document::of(settings.clone())).unwrap();
    object["version"] = json!(version);
    serde_json::to_vec(&object).unwrap()
}

fn read(bytes: &[u8]) -> PipelineSettings {
    migrate::reading(bytes, &Facts::bare(true), false).unwrap().0.document.settings
}

#[test]
fn a_file_before_the_film_reads_none_and_this_format_keeps_it() {
    assert_eq!(VERSION, chain::VIRTUAL_FILM_VERSION);
    let mut plain = filmed();
    plain.virtual_film = None;
    assert_eq!(read(&at(&plain, VERSION - 1)).virtual_film, None);
    let kept = read(&at(&filmed(), VERSION));
    assert_eq!(kept.virtual_film, filmed().virtual_film);
    // Twin: the film read back is not the bare photo.
    assert_ne!(kept.virtual_film, None);
}

#[test]
fn no_film_writes_nothing_and_a_film_writes_its_stock_by_key() {
    let mut plain = filmed();
    plain.virtual_film = None;
    let bare = serde_json::to_value(Document::of(plain)).unwrap();
    assert!(bare["settings"].get("virtualFilm").is_none());
    let written = serde_json::to_value(Document::of(filmed())).unwrap();
    assert_eq!(written["settings"]["virtualFilm"]["stock"], json!("kodak-portra-800"));
}

#[test]
fn a_build_before_the_film_refuses_its_file_as_newer() {
    // A file of the next format stands for this build as this one stands for a build before it.
    let bytes = at(&filmed(), VERSION + 1);
    assert!(matches!(migrate::reading(&bytes, &Facts::bare(true), false), Err(Refusal::Newer(v)) if v == VERSION + 1));
    // Twin: this format's own file reads.
    assert!(migrate::reading(&at(&filmed(), VERSION), &Facts::bare(true), false).is_ok());
}
