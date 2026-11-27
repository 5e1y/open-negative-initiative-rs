//! The intent of `SettingsCodec+Checks`: every migration read on a document carrying the version
//! before it, each with a twin that fails when the migration reaches too far or not at all.

use oni_settings::axis::{density_axis, pipeline};
use oni_settings::blocks::{ChromaDenoise, Gains, Sharpen, Vignette};
use oni_settings::cg::{Point, Rect};
use oni_settings::document::{self, VERSION};
use oni_settings::fields::Field;
use oni_settings::migrate::{self, chain};
use oni_settings::curves::SaturationCurve;
use oni_settings::frame::{AspectRatio, RgbPasses};
use oni_settings::migrate::before_1_0::{self, former_axis};
use oni_settings::levels::{Levels, LevelsChannel, LevelsSet};
use oni_settings::sidecar::ExifLine;
use oni_settings::migrate::{departure::FORMER_REST, exposure::GEAR};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, Date, Document, Film, Id, LegacyMachine, Medal, PipelineSettings};
use serde_json::{Map, Value, json};

#[path = "codec_checks/units.rs"]
mod units;
#[path = "codec_checks/fields.rs"]
mod fields;

type Object = Map<String, Value>;

/// `settings` as a build of format 21 and under wrote them, stamped `version`.
fn encoded(settings: &PipelineSettings, version: i64) -> Object {
    match migrate::reshape::as_stored(&Document::of(settings.clone()), version) {
        Value::Object(o) => o,
        _ => unreachable!(),
    }
}

/// The document of `settings`, stamped `version`, its settings object edited by hand.
fn aged(settings: &PipelineSettings, version: i64, edit: impl FnOnce(&mut Object)) -> Vec<u8> {
    let mut object = encoded(settings, version);
    if let Some(Value::Object(inner)) = object.get_mut("settings") {
        edit(inner);
    }
    serde_json::to_vec(&Value::Object(object)).unwrap()
}

fn decode(data: &[u8]) -> PipelineSettings {
    migrate::document(data).unwrap().document.settings
}

/// `file` says whether the document's file decodes through the mosaic, `None` for no file.
fn decode_from(data: &[u8], file: Option<bool>) -> PipelineSettings {
    match file {
        None => decode(data),
        Some(mosaic) => migrate::converted(data, &migrate::Facts { matrix: &|| Ok(None), ..migrate::Facts::bare(mosaic) }).unwrap().document.settings,
    }
}

fn full() -> PipelineSettings {
    let mut s = PipelineSettings::default();
    s.geometry.angle = 3.75;
    s.geometry.crop = Rect::new(0.1, 0.2, 0.5, 0.3);
    s.geometry.quarter_turns = 1;
    s.geometry.ratio = Some(AspectRatio::SixSeven);
    s.geometry.portrait = true;
    s.geometry.mirrored = true;
    s.geometry.mirrored_vertically = true;
    s.chroma = ChromaDenoise { radius: 4.6137, force: 0.8 };
    s.gains.stops = [1.25, -0.5, 2.125];
    // Off the resting ceiling and any round value, so a dropped key cannot round trip by luck.
    s.density_ceiling = 2.3137;
    s.dye_set = oni_settings::DyeSet::of("fujifilm-c200");
    s.levels.red.black = 0.1373;
    s.levels.red.white = 2.4477;
    s.levels.blue.mid = 0.375;
    s.levels.blue.shadows = 0.1373;
    s.levels.green.highlights = 0.8117;
    s.manual_curves.luma.points = vec![Point::new(0.0, 0.0), Point::new(0.4, 0.55), Point::new(1.0, 1.0)];
    s.print.red.exposure = 0.3137;
    s.print.blue.exposure = -0.2;
    s.saturation = SaturationCurve::converted(0.4, -0.2);
    s.vignette = Vignette { stops: -0.75 };
    s.sharpen = Sharpen { radius: 3.1373, amount: 1.5, ..Sharpen::default() };
    let mut film = Film::new("Portra 400");
    film.id = Id::parse("6F9619FF-8B86-D011-B42D-00CF4FC964FF").unwrap();
    s.film = Some(film);
    s
}

// MARK: - The envelope and the exact round trip

#[test]
fn the_envelope_reads_what_the_full_decode_reads() {
    let mut filed = PipelineSettings::default();
    let mut film = Film::new("Portra 400 · 12");
    film.manual_order = Some(3.0);
    filed.film = Some(film);
    filed.manual_order = Some(2.5);
    filed.mode = ConversionMode::Positive;
    filed.pass_of = Some("/tmp/a.ARW".into());
    filed.rgb_passes = Some(RgbPasses { red: "r.ARW".into(), green: "g.ARW".into(), blue: "b.ARW".into(), registration: None });
    let stamped = Date::from_unix_seconds(1_700_000_000.0);
    let mut whole = Document::of(filed.clone());
    (whole.imported_at, whole.label, whole.trashed_at) = (Some(stamped), Medal::Silver, Some(stamped));
    whole.exif_line = ExifLine { camera: Some("Leica M6".into()), ..ExifLine::default() };
    let text = document::encode(&whole).unwrap();
    assert!(String::from_utf8_lossy(&text).contains("\"importedAt\": \"2023-11-14T22:13:20Z\""));
    assert_eq!(document::decode(&text).unwrap(), whole);
    let light = document::envelope(&text).unwrap();
    assert_eq!((light.imported_at, light.label, light.trashed_at), (whole.imported_at, whole.label, whole.trashed_at));
    assert_eq!(light.film, whole.settings.film);
    assert_eq!(light.manual_order, whole.settings.manual_order);
    assert_eq!(light.mode, whole.settings.mode);
    assert_eq!(light.pass_of, whole.settings.pass_of);
    assert_eq!(light.leads_passes, whole.settings.rgb_passes.is_some());
    assert_eq!(light.exif, whole.exif_line);
    // The same envelope from the document as format 21 wrote it: the keys it reads never moved.
    let stored = serde_json::to_vec(&migrate::reshape::as_stored(&whole, 19)).unwrap();
    assert_eq!(document::envelope(&stored), Some(document::Envelope { format: Some(19), ..light.clone() }));
    assert_eq!(light.format, Some(VERSION));
    // Twin: another film reads another film, and absent fields read absent.
    let mut other = filed.clone();
    other.film = Some(Film::new("HP5"));
    let twin = document::envelope(&document::encode(&Document::of(other)).unwrap()).unwrap();
    assert_ne!(twin.film, light.film);
    assert_eq!((twin.label, twin.trashed_at, twin.exif.camera), (Medal::None, None, None));
}

#[test]
fn every_stage_makes_an_exact_round_trip() {
    let full = full();
    let text = document::encode_settings(&full).unwrap();
    let back = document::decode(text.as_bytes()).unwrap();
    assert_eq!(back.settings, full);
    assert_eq!(back.version, VERSION);
    assert_eq!(back.settings.sharpen.radius, 3.1373);
    let bare = document::decode(document::encode_settings(&PipelineSettings::default()).unwrap().as_bytes()).unwrap();
    assert_eq!(bare.settings.film, None);
    // Twin: one field moved by one ulp is a different document.
    let mut moved = full.clone();
    moved.gains.stops[0] = f32::from_bits(full.gains.stops[0].to_bits() + 1);
    assert_ne!(decode(document::encode_settings(&moved).unwrap().as_bytes()), full);
}

#[test]
fn an_absent_stage_or_field_reads_neutral_its_neighbours_intact() {
    let full = full();
    let relaxed = decode(&aged(&full, VERSION, |s| {
        s.remove("sharpen");
    }));
    assert_eq!(relaxed.sharpen, Sharpen::default());
    assert_eq!((relaxed.gains, &relaxed.manual_curves), (full.gains, &full.manual_curves));
    // A recursive merge, not a shallow one: a missing field keeps the rest of its stage.
    let patched = decode(&aged(&full, VERSION, |s| {
        s["levels"]["red"].as_object_mut().unwrap().remove("white");
    }));
    assert_eq!(patched.levels.red.white, LevelsSet::default().red.white);
    assert_eq!(patched.levels.red.black, full.levels.red.black);
    assert_eq!(patched.levels.blue, full.levels.blue);
    // A window from before the two added points reads them at rest; its neighbour carries its own.
    let older = decode(&aged(&full, VERSION, |s| {
        let blue = s["levels"]["blue"].as_object_mut().unwrap();
        blue.remove("shadows");
        blue.remove("highlights");
    }));
    assert!(older.levels.blue.is_window_neutral());
    assert_eq!((older.levels.blue.black, older.levels.blue.mid), (full.levels.blue.black, full.levels.blue.mid));
    assert_eq!(older.levels.green.highlights, full.levels.green.highlights);
    assert!(!older.levels.green.is_window_neutral());
}

#[test]
fn a_document_without_mode_reads_negative_and_a_spelled_mode_itself() {
    let full = full();
    let modeless = decode(&aged(&full, VERSION, |s| {
        s.remove("mode");
    }));
    assert_eq!(modeless.mode, ConversionMode::Negative);
    assert_eq!((modeless.gains, modeless.levels), (full.gains, full.levels));
    let flipped = decode(&aged(&full, VERSION, |s| {
        s.insert("mode".into(), json!("positive"));
    }));
    assert_eq!(flipped.mode, ConversionMode::Positive);
    let mut positive = full.clone();
    positive.mode = ConversionMode::Positive;
    let text = document::encode_settings(&positive).unwrap();
    assert!(text.contains("\"positive\""));
    assert_eq!(decode(text.as_bytes()), positive);
}

#[test]
fn the_crop_display_flag_stays_out_of_the_sidecar() {
    let mut bypassing = full();
    bypassing.geometry.crop_bypassed = true;
    let text = document::encode_settings(&bypassing).unwrap();
    assert!(!text.contains("cropBypassed"));
    assert!(!decode(text.as_bytes()).geometry.crop_bypassed);
}

#[test]
fn empty_reads_neutral_garbage_fails_and_the_future_is_flagged() {
    let empty = migrate::document(b"{}").unwrap();
    assert_eq!((empty.document.settings, empty.from), (migrate::departure::caught_up(0, migrate::exposure::caught_up(0, PipelineSettings::default())), 0));
    assert!(migrate::document(b"this is not JSON").is_err());
    assert!(migrate::document(b"[1, 2]").is_err());
    let future = migrate::document(&aged(&full(), VERSION + 1, |_| {}));
    assert_eq!(future, Err(migrate::Refusal::Newer(VERSION + 1)));
    // Twin: today's version is read.
    assert_eq!(migrate::document(&aged(&full(), VERSION, |_| {})).unwrap().document.settings, full());
}

// MARK: - Rests that moved

#[test]
fn the_chroma_doses_nobody_chose_retire_on_version_one() {
    let read = |radius: f32, force: f32| {
        decode(&aged(&PipelineSettings::default(), 1, |s| {
            s.insert("chroma".into(), json!({"radius": radius as f64, "force": force as f64}));
        }))
        .chroma
    };
    for (r, f) in ChromaDenoise::FORMER_RESTS {
        assert_eq!(read(r, f), ChromaDenoise::default());
    }
    // Twin: a hand's dose survives, scaled to pixels.
    let chosen = read(0.004, 0.6);
    assert_eq!(chosen.force, 0.6);
    assert!(chosen.radius > 0.0 && chosen.radius != ChromaDenoise::default().radius);
}

#[test]
fn a_positive_from_before_the_move_opens_on_the_new_rest() {
    let read = |mode: ConversionMode, levels: LevelsSet| {
        let written = PipelineSettings { mode, levels, ..PipelineSettings::default() };
        decode(&aged(&written, before_1_0::POSITIVE_REST_VERSION - 1, |_| {})).levels
    };
    let stale = LevelsSet { red: Levels::on_axis(), green: Levels::on_axis(), blue: Levels::on_axis() };
    let resting = read(ConversionMode::Positive, stale);
    assert_eq!(resting, LevelsSet::neutral(ConversionMode::Positive, density_axis::base()));
    // Twin: the same document as a negative or a monochrome comes back key for key.
    assert_eq!(read(ConversionMode::Negative, stale), stale);
    assert_eq!(read(ConversionMode::Monochrome, stale), stale);
    let mut by_hand = stale;
    by_hand.red.set_black(0.42);
    let survived = read(ConversionMode::Positive, by_hand);
    assert_eq!(survived.red.black, 0.42);
    assert_eq!(survived.green.black, Levels::resting_black(ConversionMode::Positive, density_axis::base()));
}

#[test]
fn sharpen_chroma_and_black_point_rests_move_once() {
    let sharpen = |amount: f32, version: i64| {
        let mut w = PipelineSettings::default();
        w.sharpen.amount = amount;
        decode(&aged(&w, version, |_| {})).sharpen.amount
    };
    assert_eq!(sharpen(Sharpen::FORMER_AMOUNT, before_1_0::SHARPEN_REST_VERSION - 1), Sharpen::default().amount);
    assert_eq!(sharpen(0.8, before_1_0::SHARPEN_REST_VERSION - 1), 0.8);
    assert_eq!(sharpen(Sharpen::FORMER_AMOUNT, before_1_0::SHARPEN_REST_VERSION), Sharpen::FORMER_AMOUNT);

    let chroma = |dose: ChromaDenoise, version: i64| {
        decode(&aged(&PipelineSettings { chroma: dose, ..PipelineSettings::default() }, version, |_| {})).chroma
    };
    assert_eq!(chroma(ChromaDenoise::OFF, before_1_0::CHROMA_REST_VERSION - 1), ChromaDenoise::default());
    let dosed = ChromaDenoise { radius: 1.5, force: 0.6 };
    assert_eq!(chroma(dosed, before_1_0::CHROMA_REST_VERSION - 1), dosed);
    assert_eq!(chroma(ChromaDenoise::OFF, before_1_0::CHROMA_REST_VERSION), ChromaDenoise::OFF);

    let black = |version: i64, writing: bool, value: Option<f32>| {
        decode(&aged(&PipelineSettings::default(), version, |s| {
            let print = s["print"].as_object_mut().unwrap();
            if !writing {
                print.remove("blackPoint");
            }
            if let Some(v) = value {
                print.insert("blackPoint".into(), json!(v as f64));
            }
        }))
        .print
        .black_point
    };
    assert_eq!(black(before_1_0::BLACK_POINT_REST_VERSION - 1, false, None), 0.0);
    assert_eq!(PipelineSettings::default().print.black_point, Print::RESTING_BLACK_POINT);
    assert_eq!(black(VERSION, true, None), Print::RESTING_BLACK_POINT);
    assert_eq!(black(VERSION, true, Some(-0.06)), -0.06);
    assert_eq!(black(VERSION, true, Some(-0.03)), -0.03);
}

#[test]
fn decompression_rests_move_off_the_key_and_keep_the_hand() {
    let read = |dose: Option<f32>, by_hand: Option<bool>, version: i64| {
        decode(&aged(&PipelineSettings::default(), version, |s| {
            s.remove("chromaDecompression");
            s.remove("chromaDecompressionByHand");
            if let Some(d) = dose {
                s.insert("chromaDecompression".into(), json!(d as f64));
            }
            if let Some(h) = by_hand {
                s.insert("chromaDecompressionByHand".into(), json!(h));
            }
        }))
    };
    // Version 13: a key's 0 moves to the half rest, a hand's 0 stays.
    assert_eq!(read(Some(0.0), Some(false), 13).chroma_decompression, 0.5);
    assert_eq!(read(Some(0.0), Some(true), 13).chroma_decompression, 0.0);
    // Version 12: an absent dose is the former rest 1, kept; a stored 1 is not the hand's.
    let absent = read(None, None, 12);
    assert_eq!((absent.chroma_decompression, absent.chroma_decompression_by_hand), (1.0, false));
    let off_rest = read(Some(0.3), None, 12);
    assert_eq!((off_rest.chroma_decompression, off_rest.chroma_decompression_by_hand), (0.3, true));
    // Twin: written since, a 0 is a dose and stays.
    assert_eq!(read(Some(0.0), Some(false), chain::DECOMPRESSION_MID_VERSION).chroma_decompression, 0.0);
}
