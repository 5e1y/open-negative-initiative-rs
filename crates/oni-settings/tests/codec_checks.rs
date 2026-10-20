//! The intent of `SettingsCodec+Checks`: every migration read on a document carrying the version
//! before it, each with a twin that fails when the migration reaches too far or not at all.

use oni_settings::axis::{density_axis, pipeline};
use oni_settings::blocks::{ChromaDenoise, Gains, SeparationFamily, Sharpen, Vignette, separation};
use oni_settings::cg::{Point, Rect};
use oni_settings::codec::{self, Source, VERSION};
use oni_settings::curves::SaturationCurve;
use oni_settings::density_migration;
use oni_settings::frame::{AspectRatio, Correction, RgbPasses};
use oni_settings::legacy::{self, former_axis};
use oni_settings::levels::{Levels, LevelsChannel, LevelsSet};
use oni_settings::scanner::ScannerClicks;
use oni_settings::sidecar::{ExifMetadata, RollDraft};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, Date, Film, Id, Medal, PipelineSettings, ScannerMachine};
use serde_json::{Map, Value, json};

type Object = Map<String, Value>;

fn encoded(settings: &PipelineSettings) -> Object {
    let text = codec::encode_settings(settings).unwrap();
    match serde_json::from_str(&text).unwrap() {
        Value::Object(o) => o,
        _ => unreachable!(),
    }
}

/// The document of `settings`, stamped `version`, its settings object edited by hand.
fn aged(settings: &PipelineSettings, version: i64, edit: impl FnOnce(&mut Object)) -> Vec<u8> {
    let mut object = encoded(settings);
    object.insert("version".into(), json!(version));
    if let Some(Value::Object(inner)) = object.get_mut("settings") {
        edit(inner);
    }
    serde_json::to_vec(&Value::Object(object)).unwrap()
}

fn decode(data: &[u8]) -> PipelineSettings {
    codec::decode(data).unwrap().settings
}

fn decode_from(data: &[u8], mosaic: bool, plain: bool) -> PipelineSettings {
    codec::decode_with(data, &Source { mosaic: &|| mosaic, plain: &|| plain }, false).unwrap().settings
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
    s.separation = 0.37;
    s.separation_family = Some(SeparationFamily::FujiC41);
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
    let typed = ExifMetadata { camera: "Leica M6".into(), ..ExifMetadata::default() };
    let draft = RollDraft {
        id: Id::new(),
        step: "Frames".into(),
        film: ConversionMode::Positive,
        capture: "camera".into(),
        pass_order: None,
        flat_field: None,
        base: None,
        window: None,
        ticked: true,
        flagged: false,
    };
    let text = codec::encode(&filed, Some(stamped), Some(Medal::Silver), Some(typed), Some(draft), Some(stamped)).unwrap();
    assert!(text.contains("\"importedAt\" : \"2023-11-14T22:13:20Z\""), "{text}");
    let whole = codec::decode(text.as_bytes()).unwrap();
    let light = codec::envelope(text.as_bytes()).unwrap();
    assert_eq!(
        (light.imported_at, light.label, &light.exif, &light.roll_draft, light.trashed_at),
        (whole.imported_at, whole.label, &whole.exif, &whole.roll_draft, whole.trashed_at)
    );
    assert_eq!(light.film, whole.settings.film);
    assert_eq!(light.manual_order, whole.settings.manual_order);
    assert_eq!(light.mode, whole.settings.mode);
    assert_eq!(light.pass_of, whole.settings.pass_of);
    assert_eq!(light.leads_passes, whole.settings.rgb_passes.is_some());
    // Twin: another film reads another film, and absent fields read absent.
    let mut other = filed.clone();
    other.film = Some(Film::new("HP5"));
    let twin = codec::envelope(codec::encode(&other, Some(stamped), None, None, None, None).unwrap().as_bytes()).unwrap();
    assert_ne!(twin.film, light.film);
    assert_eq!((twin.label, twin.trashed_at), (Medal::None, None));
}

#[test]
fn every_stage_makes_an_exact_round_trip() {
    let full = full();
    let text = codec::encode_settings(&full).unwrap();
    let back = codec::decode(text.as_bytes()).unwrap();
    assert_eq!(back.settings, full);
    assert_eq!(back.version, VERSION);
    assert_eq!(back.settings.sharpen.radius, 3.1373);
    let bare = codec::decode(codec::encode_settings(&PipelineSettings::default()).unwrap().as_bytes()).unwrap();
    assert_eq!(bare.settings.film, None);
    // Twin: one field moved by one ulp is a different document.
    let mut moved = full.clone();
    moved.gains.stops[0] = f32::from_bits(full.gains.stops[0].to_bits() + 1);
    assert_ne!(decode(codec::encode_settings(&moved).unwrap().as_bytes()), full);
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
    let text = codec::encode_settings(&positive).unwrap();
    assert!(text.contains("\"positive\""));
    assert_eq!(decode(text.as_bytes()), positive);
}

#[test]
fn the_crop_display_flag_stays_out_of_the_sidecar() {
    let mut bypassing = full();
    bypassing.geometry.crop_bypassed = true;
    let text = codec::encode_settings(&bypassing).unwrap();
    assert!(!text.contains("cropBypassed"));
    assert!(!decode(text.as_bytes()).geometry.crop_bypassed);
}

#[test]
fn empty_reads_neutral_garbage_fails_and_the_future_is_flagged() {
    let empty = codec::decode(b"{}").unwrap();
    assert_eq!((empty.settings, empty.version), (PipelineSettings::default(), 0));
    assert!(codec::decode(b"this is not JSON").is_err());
    assert!(codec::decode(b"[1, 2]").is_err());
    let future = codec::decode(&aged(&full(), VERSION + 1, |_| {})).unwrap();
    assert!(future.is_from_future());
    // Twin: today's version is not.
    assert!(!codec::decode(&aged(&full(), VERSION, |_| {})).unwrap().is_from_future());
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
        decode(&aged(&written, legacy::POSITIVE_REST_VERSION - 1, |_| {})).levels
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
fn separation_rests_move_only_where_nobody_chose() {
    let read = |dose: f32, by_hand: bool, version: i64| {
        let written = PipelineSettings { separation: dose, separation_by_hand: by_hand, ..PipelineSettings::default() };
        decode(&aged(&written, version, |_| {})).separation
    };
    let before_7 = legacy::SEPARATION_REST_VERSION - 1;
    assert_eq!(read(separation::FORMER_BASE, false, before_7), separation::FORMER_REST);
    assert_eq!(read(1.6, false, before_7), 1.6);
    // Twin: 100 % written after the move stays 100 %.
    assert_eq!(read(separation::FORMER_BASE, false, legacy::SEPARATION_REST_VERSION), separation::FORMER_BASE);
    let previous = codec::SEPARATION_LITERATURE_VERSION - 1;
    assert_eq!(read(separation::PREVIOUS_BASE, false, previous), separation::BASE);
    // Twin: the hand's 18 %, or one written after the move, stays.
    assert_eq!(read(separation::PREVIOUS_BASE, true, previous), separation::PREVIOUS_BASE);
    assert_eq!(read(separation::PREVIOUS_BASE, false, codec::SEPARATION_LITERATURE_VERSION), separation::PREVIOUS_BASE);
}

#[test]
fn the_separation_chain_before_1_0_lands_where_each_step_says() {
    // Before 7, 1 becomes the former rest 0.67; before 14 that rest is not the hand's; then 0.6 is not 0.18.
    let written = PipelineSettings { separation: separation::FORMER_BASE, ..PipelineSettings::default() };
    let read = decode(&aged(&written, 6, |s| {
        s.remove("separationByHand");
    }));
    assert_eq!((read.separation, read.separation_by_hand), (separation::FORMER_REST, false));
    let read = decode(&aged(&written, 13, |s| {
        s.remove("separationByHand");
    }));
    assert_eq!((read.separation, read.separation_by_hand), (separation::FORMER_BASE, true));
}

#[test]
fn sharpen_chroma_and_black_point_rests_move_once() {
    let sharpen = |amount: f32, version: i64| {
        let mut w = PipelineSettings::default();
        w.sharpen.amount = amount;
        decode(&aged(&w, version, |_| {})).sharpen.amount
    };
    assert_eq!(sharpen(Sharpen::FORMER_AMOUNT, legacy::SHARPEN_REST_VERSION - 1), Sharpen::default().amount);
    assert_eq!(sharpen(0.8, legacy::SHARPEN_REST_VERSION - 1), 0.8);
    assert_eq!(sharpen(Sharpen::FORMER_AMOUNT, legacy::SHARPEN_REST_VERSION), Sharpen::FORMER_AMOUNT);

    let chroma = |dose: ChromaDenoise, version: i64| {
        decode(&aged(&PipelineSettings { chroma: dose, ..PipelineSettings::default() }, version, |_| {})).chroma
    };
    assert_eq!(chroma(ChromaDenoise::OFF, legacy::CHROMA_REST_VERSION - 1), ChromaDenoise::default());
    let dosed = ChromaDenoise { radius: 1.5, force: 0.6 };
    assert_eq!(chroma(dosed, legacy::CHROMA_REST_VERSION - 1), dosed);
    assert_eq!(chroma(ChromaDenoise::OFF, legacy::CHROMA_REST_VERSION), ChromaDenoise::OFF);

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
    assert_eq!(black(legacy::BLACK_POINT_REST_VERSION - 1, false, None), 0.0);
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
    assert_eq!(read(Some(0.0), Some(false), codec::DECOMPRESSION_MID_VERSION).chroma_decompression, 0.0);
}

// MARK: - Unit changes, whose order is load-bearing

#[test]
fn the_per_channel_windows_move_onto_the_density_axis_before_the_defaults() {
    let mut placed = PipelineSettings::default();
    placed.levels.red = Levels { black: 0.0731, white: 0.8137, mid: 0.3117, ..Levels::default() };
    placed.levels.green = Levels { black: 0.1279, white: 0.7411, mid: 0.6413, ..Levels::default() };
    placed.levels.blue = Levels { black: 0.2113, white: 0.9337, mid: 0.5, ..Levels::default() };
    placed.gains.stops = [1.5, -0.5, 2.125];
    let scaled = density_migration::migrate(&placed.levels);
    for stamp in [0, legacy::DENSITY_AXIS_VERSION - 1, legacy::DENSITY_AXIS_VERSION, VERSION + 1] {
        let read = decode(&aged(&placed, stamp, |_| {}));
        let wanted = if stamp < legacy::DENSITY_AXIS_VERSION { scaled } else { placed.levels };
        assert_eq!(read.levels, wanted, "version {stamp}");
    }
    // Twin: the placed windows really move under the scaling.
    assert_ne!(placed.levels, scaled);
    let old = aged(&placed, legacy::DENSITY_AXIS_VERSION - 1, |_| {});
    let read = decode(&old);
    for c in LevelsChannel::PER_CHANNEL {
        assert_eq!(read.levels.get(c).mid, placed.levels.get(c).mid);
    }
    assert_eq!((read.gains, &read.manual_curves, read.print), (placed.gains, &placed.manual_curves, placed.print));

    let missing = aged(&placed, legacy::DENSITY_AXIS_VERSION - 1, |s| {
        s["levels"].as_object_mut().unwrap().remove("red");
    });
    let holed = decode(&missing);
    assert_eq!(holed.levels.red, LevelsSet::default().red);
    assert_eq!((holed.levels.green, holed.levels.blue), (scaled.green, scaled.blue));
    // Twin: the forbidden order, defaults then scale, triples the absent default.
    let stored: Value = serde_json::from_slice(&missing).unwrap();
    let completed = codec::merged(&stored, codec::neutral_document());
    let wrong = legacy::in_density(completed.as_object().unwrap().clone());
    let wrong: codec::SettingsDocument = serde_json::from_value(Value::Object(wrong)).unwrap();
    assert_eq!(wrong.settings.levels.red.white, LevelsSet::default().red.white * pipeline::DMAX);

    let partial = decode(&aged(&placed, legacy::DENSITY_AXIS_VERSION - 1, |s| {
        s["levels"]["blue"].as_object_mut().unwrap().remove("white");
    }));
    assert_eq!(partial.levels.blue.white, LevelsSet::default().blue.white);
    assert_eq!(partial.levels.blue.black, scaled.blue.black);
}

#[test]
fn the_two_radii_move_onto_pixels_before_the_defaults() {
    let side = codec::RADIUS_REFERENCE_SIDE as f32;
    let document = |chroma: Option<(f32, f32)>, sharpen: Option<f32>, stamp: i64, holing: bool| {
        aged(&PipelineSettings::default(), stamp, |s| {
            if let Some((r, f)) = chroma {
                s.insert("chroma".into(), json!({"radius": r as f64, "force": f as f64}));
            }
            let block = s["sharpen"].as_object_mut().unwrap();
            if holing {
                block.remove("radius");
            } else if let Some(r) = sharpen {
                block.insert("radius".into(), json!(r as f64));
            }
        })
    };
    let hand = (0.0005f32, 0.62f32);
    let sharpened = 0.00037f32;
    for stamp in [0, legacy::RADIUS_PIXELS_VERSION - 1, legacy::RADIUS_PIXELS_VERSION, VERSION + 1] {
        let read = decode(&document(Some(hand), Some(sharpened), stamp, false));
        let migrates = stamp < legacy::RADIUS_PIXELS_VERSION;
        let wanted = if migrates { (hand.0 * side, sharpened * side) } else { (hand.0, sharpened) };
        assert!((read.chroma.radius - wanted.0).abs() < 1e-4, "version {stamp}: {}", read.chroma.radius);
        assert!((read.sharpen.radius - wanted.1).abs() < 1e-4, "version {stamp}: {}", read.sharpen.radius);
    }
    let holed = document(Some(hand), None, legacy::RADIUS_PIXELS_VERSION - 1, true);
    let read = decode(&holed);
    assert_eq!(read.sharpen.radius, Sharpen::default().radius);
    assert!((read.chroma.radius - hand.0 * side).abs() < 1e-4);
    // Twin: the forbidden order puts the absent default on the track's stop.
    let stored: Value = serde_json::from_slice(&holed).unwrap();
    let completed = codec::merged(&stored, codec::neutral_document());
    let wrong = legacy::in_pixels(completed.as_object().unwrap().clone(), false);
    let wrong: codec::SettingsDocument = serde_json::from_value(Value::Object(wrong)).unwrap();
    assert_eq!(wrong.settings.sharpen.radius, Sharpen::radius_range().1);
    assert_ne!(wrong.settings.sharpen.radius, Sharpen::default().radius);

    for (r, f) in ChromaDenoise::FORMER_RESTS {
        assert_eq!(decode(&document(Some((r, f)), Some(sharpened), 1, false)).chroma, ChromaDenoise::default());
    }
    let former = decode(&document(None, Some(Sharpen::FORMER_REST), legacy::RADIUS_PIXELS_VERSION - 1, false));
    assert_eq!(former.sharpen.radius, Sharpen::RADIUS_ANCHOR);
    // Twin: one percent off that floor is a decision, and is scaled.
    let nearby = decode(&document(None, Some(Sharpen::FORMER_REST * 1.01), legacy::RADIUS_PIXELS_VERSION - 1, false));
    assert!(nearby.sharpen.radius != Sharpen::RADIUS_ANCHOR && nearby.sharpen.radius > 0.0);
    let past = decode(&document(Some((0.01, 1.0)), Some(0.002), legacy::RADIUS_PIXELS_VERSION - 1, false));
    assert_eq!(past.chroma.radius, ChromaDenoise::RADIUS_RANGE.1);
    assert_eq!(past.sharpen.radius, Sharpen::radius_range().1);
}

#[test]
fn a_mosaic_gain_shifts_for_the_decode_change_and_a_scanner_gain_never() {
    let hand = [0.6f32, -0.3, 0.9];
    let delta = legacy::NEUTRAL_DECODE_RATIO.map(f32::log2);
    let expected: [f32; 3] = std::array::from_fn(|i| hand[i] - delta[i]);
    for mode in ConversionMode::ALL {
        for stamp in [0, legacy::NEUTRAL_DECODE_GAINS_VERSION - 1, legacy::NEUTRAL_DECODE_GAINS_VERSION, VERSION + 1] {
            let mut written = PipelineSettings { mode, ..PipelineSettings::default() };
            written.gains.stops = hand;
            let data = aged(&written, stamp, |_| {});
            let source = Source { mosaic: &|| true, plain: &|| false };
            let mosaic = codec::decode_with(&data, &source, true).unwrap().settings;
            let migrates = stamp < legacy::NEUTRAL_DECODE_GAINS_VERSION;
            assert_eq!(mosaic.gains.stops, if migrates { expected } else { hand }, "{mode:?} v{stamp}");
            // Twin: read as scanner-sourced, the same document keeps its gains at every version.
            assert_eq!(decode(&data).gains.stops, hand);
        }
    }
    let mut near = PipelineSettings::default();
    near.gains.stops = [-2.9, 0.0, 2.9];
    let read = codec::decode_with(&aged(&near, 0, |_| {}), &Source { mosaic: &|| true, plain: &|| false }, true)
        .unwrap()
        .settings;
    assert_eq!((read.gains.stops[0], read.gains.stops[2]), (Gains::RANGE.0, Gains::RANGE.1));
    // Twin: green's small shift stays inside the track.
    assert!((read.gains.stops[1] + delta[1]).abs() < 1e-4);
    assert!(read.gains.stops[1] > Gains::RANGE.0 && read.gains.stops[1] < Gains::RANGE.1);
}

#[test]
fn a_mosaic_raw_never_balanced_rests_knee_free_and_a_balanced_one_keeps_its_axis() {
    let stamped = |build: &dyn Fn(&mut PipelineSettings), version: i64| {
        let mut s = PipelineSettings::default();
        build(&mut s);
        aged(&s, version, |inner| {
            inner.remove("decodedFromMosaic");
        })
    };
    let v = legacy::MOSAIC_AXIS_VERSION - 1;
    let read = |data: &[u8], mosaic: bool| decode_from(data, mosaic, false);
    let untouched = stamped(&|_| {}, v);
    let rested = read(&untouched, true);
    assert_eq!(rested.density_ceiling, density_axis::MOSAIC_REST);
    assert!(rested.decoded_from_mosaic);
    // Twin: the same bytes beside a scanner file move nothing.
    let scanner = read(&untouched, false);
    assert_eq!((scanner.density_ceiling, scanner.decoded_from_mosaic), (density_axis::base(), false));
    let kept = read(
        &stamped(
            &|s| {
                s.levels.blue.white = 2.21;
                s.levels.red.black = 0.37;
            },
            v,
        ),
        true,
    );
    assert!((kept.density_ceiling - former_axis(density_axis::base())).abs() < 1e-6);
    assert_eq!((kept.levels.blue.white, kept.levels.red.black), (2.21, 0.37));
    let former = density_axis::pedestal(density_axis::base()) - legacy::FORMER_SHADOW_BIAS;
    let rebuilt = density_axis::pedestal(former_axis(density_axis::base()));
    assert!((rebuilt - former).abs() / former < 1e-5, "{rebuilt} against {former}");
    let dialled = read(
        &stamped(
            &|s| {
                s.density_ceiling = 2.3;
                s.levels.green.white = 2.4;
            },
            v,
        ),
        true,
    );
    assert!((dialled.density_ceiling - former_axis(2.3)).abs() < 1e-6 && dialled.density_ceiling > 2.9);
    let past = read(
        &stamped(
            &|s| {
                s.density_ceiling = 2.5;
                s.levels.green.white = 2.4;
            },
            v,
        ),
        true,
    );
    assert_eq!(past.density_ceiling, density_axis::MOSAIC_REST);
    let current = read(&stamped(&|_| {}, legacy::MOSAIC_AXIS_VERSION), true);
    assert_eq!(current.density_ceiling, density_axis::base());
    let slide = read(
        &stamped(
            &|s| {
                s.mode = ConversionMode::Positive;
                s.levels = LevelsSet::neutral(ConversionMode::Positive, density_axis::base());
            },
            v,
        ),
        true,
    );
    assert!((slide.density_ceiling - former_axis(density_axis::base())).abs() < 1e-6);
    assert_eq!(slide.levels.green.black, Levels::resting_black(ConversionMode::Positive, density_axis::base()));
    let graded = read(&stamped(&|s| s.gains.stops = [0.4, 0.0, -0.3], v), true);
    assert!((graded.density_ceiling - former_axis(density_axis::base())).abs() < 1e-6);
    let healed = read(&stamped(&|s| s.corrections = vec![Correction::new(vec![Point::new(0.4, 0.6)])], v), true);
    assert!(healed.lens_corrected && !rested.lens_corrected);
}

#[test]
fn a_scan_from_before_keeps_its_converted_primaries() {
    let stamped = |version: i64| {
        let mut s = PipelineSettings::default();
        s.levels.red.black = 0.41;
        aged(&s, version, |inner| {
            inner.remove("primariesConverted");
        })
    };
    let before = codec::PRIMARIES_HELD_VERSION - 1;
    assert!(decode_from(&stamped(before), false, true).primaries_converted);
    // Twin: a RAW's sidecar from before, and a scan's written since, are left alone.
    assert!(!decode_from(&stamped(before), true, false).primaries_converted);
    assert!(!decode_from(&stamped(codec::PRIMARIES_HELD_VERSION), false, true).primaries_converted);
}

#[test]
fn a_scanner_edit_from_before_reads_its_counts_doubled() {
    let stamped = |version: i64| {
        let settings = PipelineSettings { scanner: ScannerClicks::new(3, -5, 7, -12, 4), ..PipelineSettings::default() };
        aged(&settings, version, |_| {})
    };
    let before = decode_from(&stamped(codec::SCANNER_HALF_STEP_VERSION - 1), false, false).scanner;
    assert_eq!(before, ScannerClicks::new(6, -10, 14, -24, 4));
    // Twin: written since, the counts read as stored.
    let since = decode_from(&stamped(codec::SCANNER_HALF_STEP_VERSION), false, false).scanner;
    assert_eq!(since, ScannerClicks::new(3, -5, 7, -12, 4));
    let older = decode_from(&stamped(codec::ONI_MACHINE_VERSION - 1), false, false).scanner_machine;
    let newer = decode_from(&stamped(codec::ONI_MACHINE_VERSION), false, false).scanner_machine;
    assert_eq!((older, newer), (ScannerMachine::Frontier, ScannerMachine::Oni));
}

// MARK: - The former tone chain

#[test]
fn a_former_document_is_fitted_onto_lighting_and_a_current_one_is_not() {
    // A version 8 document with no Lighting block, the former linked window and exposure.
    let former = |version: i64, with_print: bool| {
        aged(&PipelineSettings::default(), version, |s| {
            if !with_print {
                s.remove("print");
            }
            s.insert("exposure".into(), json!(0.5));
            s["levels"].as_object_mut().unwrap().insert("linked".into(), json!({"black": 0.05, "white": 0.92, "mid": 0.6}));
        })
    };
    let fitted = decode(&former(legacy::LIGHTING_VERSION - 1, false));
    assert_ne!(fitted.print, Print::default(), "the fit placed Lighting");
    assert_eq!(fitted.print.black_point, 0.0);
    assert!(fitted.print.exposure > 0.0, "a brighter former exposure reads brighter: {}", fitted.print.exposure);
    // Twin: the same keys beside a Lighting block, or written since, are left to the decoder.
    assert_eq!(decode(&former(legacy::LIGHTING_VERSION - 1, true)).print, Print::default());
    assert_eq!(decode(&former(legacy::LIGHTING_VERSION, false)).print, Print::default());
    // And a read that does not catch up leaves the former document unfitted.
    let source = Source::none();
    let raw = codec::decode_with(&former(legacy::LIGHTING_VERSION - 1, false), &source, false).unwrap();
    assert_eq!(raw.settings.print, Print::default());
}

// MARK: - Families

#[test]
fn every_field_lives_in_exactly_one_family() {
    let keys: Vec<String> = {
        let mut s = full();
        s.sheet = Some(oni_settings::frame::ContactSheetGrid {
            columns: 1,
            rows: 1,
            frames: 1,
            tile: oni_settings::cg::Size { width: 1.0, height: 1.0 },
            margin: 0.0,
            windows: None,
        });
        s.manual_order = Some(1.0);
        s.source_encoding = Some(oni_settings::SourceEncoding::Linear);
        s.rgb_passes = Some(RgbPasses { red: "r".into(), green: "g".into(), blue: "b".into(), registration: None });
        s.pass_of = Some("p".into());
        s.roll_reference = Some(oni_settings::frame::RollReference {
            id: Id::new(),
            stops: [0.0; 3],
            black: [0.0; 3],
            white: [3.0; 3],
            base: None,
            median: None,
        });
        s.balance_method = Some("classic".into());
        s.balance_filters = Some([0.0; 3]);
        encoded(&s)["settings"].as_object().unwrap().keys().cloned().collect()
    };
    let families = |names: &[&str]| -> Vec<String> {
        let mut all: Vec<String> = oni_settings::settings::look_settings::FIELDS
            .iter()
            .chain(oni_settings::settings::look_settings::FRAME_FIELDS.iter())
            .chain(names.iter())
            .map(|s| s.to_string()).collect();
        all.sort();
        all
    };
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys.len(), 42);
    assert_eq!(families(&oni_settings::settings::roll_settings::FIELD_NAMES), sorted);
    // Twin: a family missing one field no longer covers the document.
    let short = &oni_settings::settings::roll_settings::FIELD_NAMES[1..];
    assert_ne!(families(short), sorted);
}

#[test]
fn the_rest_is_the_import_state_of_each_mode() {
    let negative = PipelineSettings::rest(ConversionMode::Negative, true);
    assert_eq!(negative.density_ceiling, density_axis::MOSAIC_REST);
    assert_eq!(negative.print.exposure, Print::RESTING_EXPOSURE);
    assert!(negative.decoded_from_mosaic);
    assert_eq!(negative.levels, LevelsSet::neutral(ConversionMode::Negative, density_axis::MOSAIC_REST));
    let scan = PipelineSettings::rest(ConversionMode::Negative, false);
    assert_eq!(scan.density_ceiling, density_axis::base());
    let slide = PipelineSettings::rest(ConversionMode::Positive, true);
    assert_eq!(slide.density_ceiling, density_axis::base());
    assert_eq!(slide.print.exposure, 0.0);
    assert_eq!(slide.levels.red.black, Levels::resting_black(ConversionMode::Positive, density_axis::base()));
    let mono = PipelineSettings::rest(ConversionMode::Monochrome, false);
    assert_eq!(mono.print.exposure, Print::RESTING_EXPOSURE);
    // Twin: the rest is not the bare struct, whose Lighting is unexposed.
    assert_ne!(negative, PipelineSettings::default());
    assert_ne!(PipelineSettings::rest(ConversionMode::Negative, false), PipelineSettings::rest(ConversionMode::Negative, true));
}

#[test]
fn a_former_balance_lands_in_the_filters_and_its_greys_render_as_they_did() {
    use oni_settings::curves::CurveSet;
    use oni_settings::lighting_catch_up::{self, Curves, display, former_pixel, new_pixel};
    use oni_settings::tone::Tone;
    let mut raw = Object::new();
    let data = aged(&PipelineSettings::default(), legacy::LIGHTING_VERSION - 1, |s| {
        s.remove("print");
        s.insert("exposure".into(), json!(1.2));
        s.insert("toneBalance".into(), json!({"white": {"colour": [0.8, 0, -1.6]}, "shadows": {"colour": [0, 0.3, 0]}}));
        raw = s.clone();
    });
    let read = decode(&data);
    let f = lighting_catch_up::former(&Value::Object(raw.clone())).expect("a former document");
    let started = std::time::Instant::now();
    let fit = lighting_catch_up::fitted(&f, &CurveSet::default(), &CurveSet::default(), [1.0; 3]);
    eprintln!("fit: {:.1} ms, worst residual {:.4}", started.elapsed().as_secs_f64() * 1000.0, fit.worst);
    // The kernel replica on windows set to `n = D / 2`, so a grey's window output is known.
    let n = |t: f32| {
        -(t + density_axis::pedestal(density_axis::base())).max(density_axis::log_guard(density_axis::base())).log10() / 2.0
    };
    let identity = Curves::new(&CurveSet::default());
    let bare = PipelineSettings { print: Print::default(), density: 0.0, ..read.clone() };
    let (mut worst, mut unmigrated) = (0f32, 0f32);
    for t in [0.02f32, 0.08, 0.2, 0.45] {
        let target = former_pixel([n(t); 3], &f, &identity, &identity);
        let got = new_pixel([n(t); 3], &read.print, read.density, [1.0; 3], false, &identity, &identity);
        let plain = new_pixel([n(t); 3], &bare.print, bare.density, [1.0; 3], false, &identity, &identity);
        for c in 0..3 {
            worst = worst.max((display(got[c]) - display(target[c])).abs());
            unmigrated = unmigrated.max((display(plain[c]) - display(target[c])).abs());
        }
    }
    eprintln!("greys: worst {worst:.4}, unmigrated {unmigrated:.4}");
    assert!(worst < 0.07, "{worst}");
    // Twin: left on Lighting's rest the same greys stand apart.
    assert!(unmigrated > 0.08, "{unmigrated}");
    let parted = !read.print.red.is_neutral() || !read.print.green.is_neutral() || !read.print.blue.is_neutral();
    assert!(parted && !read.print.master().is_neutral());

    let mut own = raw.clone();
    own.insert("print".into(), json!({"exposure": 0.5}));
    let mut current = encoded(&PipelineSettings::default());
    current.insert("version".into(), json!(legacy::LIGHTING_VERSION - 1));
    current.insert("settings".into(), Value::Object(own));
    let kept = decode(&serde_json::to_vec(&Value::Object(current)).unwrap());
    assert_eq!(kept.print.exposure, 0.5);
    assert_eq!((kept.print.red, kept.print.blue), (Tone::NEUTRAL, Tone::NEUTRAL));
    assert!(lighting_catch_up::former(&json!({})).is_none());
}

#[test]
fn a_document_from_before_folds_its_scanner_look_and_one_since_carries_none() {
    let stored = PipelineSettings { density: 0.1, ..PipelineSettings::default() };
    let read = |version: i64| {
        decode(&aged(&stored, version, |s| {
            s.insert("look".into(), json!({"model": "frontier", "amount": 1.0}));
        }))
    };
    let older = read(codec::LOOK_FOLDED_VERSION - 1);
    assert!((older.density - 0.25).abs() < 1e-6, "{}", older.density);
    assert!(!older.manual_curves.red.is_neutral());
    // Twin: written since, the retired key is read as it stands, which is not at all.
    let newer = read(codec::LOOK_FOLDED_VERSION);
    assert_eq!(newer.density, 0.1);
    assert!(newer.manual_curves.is_neutral());
}
