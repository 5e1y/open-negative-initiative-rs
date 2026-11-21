//! A digital photo's film in a sidecar, and a format 37 digital photo opening on the negative's pipeline: on its
//! film, every edit with an equivalent kept, its white, picture levels and Black and White dropped; a filmed one's
//! hand moves of its film's reference kept as moves of the stock's own. Each verdict with a twin.

use oni_settings::document::VERSION;
use oni_settings::migrate::digital_film::FORMER_PORTRA_800;
use oni_settings::migrate::{self, Facts, Refusal, chain};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, Document, Medal, PipelineSettings, VirtualStock};
use serde_json::{Value, json};

fn at(settings: &PipelineSettings, version: i64) -> Value {
    let mut object = serde_json::to_value(Document::of(settings.clone())).unwrap();
    object["version"] = json!(version);
    object
}

fn read(document: &Value) -> Document {
    migrate::reading(&serde_json::to_vec(document).unwrap(), &Facts::bare(true), false).unwrap().0.document
}

/// A format 37 digital photo on no film, edited everywhere a hand can: its white, picture levels and Black and
/// White, Lighting, curves, mixer, saturation, effects, framing, a dust stroke, a medal and an EXIF line.
fn plain_37() -> Value {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.mode = ConversionMode::Digital;
    s.virtual_film = None;
    (s.print.exposure, s.print.contrast, s.print.shadows) = (0.4, 0.2, -0.1);
    s.density = 0.15;
    s.manual_curves.linked.add(oni_settings::cg::Point::new(0.3, 0.4));
    s.spectrogram.hue[2] = 0.2;
    s.secret_sauce = 0.3;
    (s.vignette.stops, s.grain.amount, s.halation.amount, s.lens_correction.distortion) = (0.4, 0.5, 0.3, 20.0);
    s.geometry.angle = 1.5;
    s.corrections.push(oni_settings::frame::Correction { id: oni_settings::Id::new(), stroke: vec![oni_settings::cg::Point::new(0.2, 0.3)], radius: 9.0, gesture: None });
    s.print.set_filters([0.3, 0.0, -0.3]);
    let mut doc = at(&s, 37);
    doc["label"] = json!("gold");
    doc["exifLine"] = json!({ "camera": "Lumix" });
    let settings = doc["settings"].as_object_mut().unwrap();
    settings.insert("white".into(), json!({ "kelvin": 4300.0, "tint": 5.0 }));
    settings.insert("shotWhite".into(), json!({ "kelvin": 5210.0, "tint": 12.9 }));
    settings.insert("blackAndWhite".into(), json!(true));
    settings.insert("pictureLevels".into(), json!({ "luma": { "black": 0.1, "white": 0.9, "mid": 0.4, "shadows": 0.25, "highlights": 0.75 } }));
    doc
}

#[test]
fn a_digital_photo_of_format_37_opens_on_its_film_its_edits_kept_and_its_white_dropped() {
    assert_eq!(VERSION, chain::STOCK_DYES_VERSION);
    let stored = plain_37();
    let before: PipelineSettings = serde_json::from_value(stored["settings"].clone()).unwrap();
    let doc = read(&stored);
    let s = &doc.settings;
    let reference = VirtualStock::KodakPortra800.reference();
    assert_eq!((s.mode, s.virtual_film.map(|f| (f.stock, f.exposure))), (ConversionMode::Digital, Some((VirtualStock::KodakPortra800, 0.0))));
    assert_eq!((s.gains.stops, s.levels, s.dye_set, s.separation), (reference.stops, reference.levels(), VirtualStock::KodakPortra800.dye_set(), 1.0));
    assert_eq!(s.print.exposure, 0.4 + Print::RESTING_EXPOSURE, "Lighting's move kept on the negative's rest");
    assert_eq!((s.print.contrast, s.print.shadows, s.density, s.print.filters()), (0.2, -0.1, 0.15, [0.0; 3]));
    assert_eq!((&s.manual_curves, s.spectrogram, s.secret_sauce, s.vignette, s.grain, s.halation), (&before.manual_curves, before.spectrogram, 0.3, before.vignette, before.grain, before.halation));
    assert_eq!((s.lens_correction, s.geometry, &s.corrections), (before.lens_correction, before.geometry, &before.corrections));
    assert_eq!((doc.label, doc.exif_line.camera.as_deref()), (Medal::Gold, Some("Lumix")));
    let written = serde_json::to_value(&doc).unwrap();
    for gone in ["white", "shotWhite", "blackAndWhite", "pictureLevels"] {
        assert!(written["settings"].get(gone).is_none(), "{gone} is dropped");
    }
    // Twin: the same photo stored at this format reads as it stands, no rest moved.
    let current = read(&at(&before, VERSION));
    assert_eq!((current.settings.print.exposure, current.settings.gains), (0.4, before.gains));
}

#[test]
fn a_filmed_photo_of_format_37_keeps_its_hand_moves_on_the_stocks_reference() {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
    s.print.exposure = 0.0;
    let mut doc = at(&s, 37);
    let former = FORMER_PORTRA_800;
    let moved = |v: [f32; 3], by: f32| v.map(|x| x + by);
    doc["settings"]["virtualFilm"] = json!({
        "stock": "kodak-portra-800",
        "exposure": 0.7,
        "stops": moved(former.stops, 0.1),
        "black": moved(former.black, 0.02),
        "white": former.white,
    });
    let s = read(&doc).settings;
    let reference = VirtualStock::KodakPortra800.reference();
    assert_eq!(s.film().map(|f| f.exposure), Some(0.7));
    for c in 0..3 {
        assert!((s.gains.stops[c] - reference.stops[c] - 0.1).abs() < 1e-6, "{:?}", s.gains.stops);
    }
    let want = oni_settings::virtual_film::StockReference { stops: s.gains.stops, black: moved(reference.black, 0.02), white: reference.white }.levels();
    for (got, want) in s.levels.windows().into_iter().zip(want.windows()) {
        assert!((got.black - want.black).abs() < 1e-5 && (got.white - want.white).abs() < 1e-5, "{got:?} {want:?}");
    }
    assert_eq!(s.print.exposure, Print::RESTING_EXPOSURE);
    // Twin: read as a photo on no film, the hand's moves are lost to the stock's own reference.
    let mut bare = doc.clone();
    bare["settings"].as_object_mut().unwrap().remove("virtualFilm");
    assert_eq!(read(&bare).settings.gains.stops, reference.stops);
}

#[test]
fn a_film_frame_of_format_37_opens_as_it_was() {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    (s.print.exposure, s.gains.stops) = (0.9, [0.2, 0.0, -0.1]);
    let read_back = read(&at(&s, 37)).settings;
    assert_eq!(read_back, s);
    // Twin: the same frame as a digital photo moves onto its film.
    let digital = PipelineSettings { mode: ConversionMode::Digital, ..s };
    assert_ne!(read(&at(&digital, 37)).settings.gains, digital.gains);
}

#[test]
fn a_film_writes_its_stock_by_key_and_a_build_before_refuses_the_file_as_newer() {
    let filmed = PipelineSettings::rest(ConversionMode::Digital, true);
    let written = serde_json::to_value(Document::of(filmed.clone())).unwrap();
    assert_eq!(written["settings"]["virtualFilm"]["stock"], json!("kodak-portra-800"));
    let bytes = serde_json::to_vec(&at(&filmed, VERSION + 1)).unwrap();
    assert!(matches!(migrate::reading(&bytes, &Facts::bare(true), false), Err(Refusal::Newer(v)) if v == VERSION + 1));
    // Twin: this format's own file reads, and a negative writes no film.
    assert!(migrate::reading(&serde_json::to_vec(&at(&filmed, VERSION)).unwrap(), &Facts::bare(true), false).is_ok());
    let negative = serde_json::to_value(Document::of(PipelineSettings::rest(ConversionMode::Negative, true))).unwrap();
    assert!(negative["settings"].get("virtualFilm").is_none());
}
