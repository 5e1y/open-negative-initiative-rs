//! The former tone chain fitted onto Lighting, and the fields a document writes, reads at rest and retires.

use super::*;

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
    let fitted = decode(&former(before_1_0::LIGHTING_VERSION - 1, false));
    assert_ne!(fitted.print, Print::default(), "the fit placed Lighting");
    assert_eq!(fitted.print.black_point, 0.0);
    assert!(FORMER_REST + GEAR * fitted.print.exposure > 0.0, "a brighter former exposure reads brighter");
    // Twin: the same keys beside a Lighting block, or written since, are left to the decoder.
    let (kept, bare) = (migrate::departure::read_back(1, PipelineSettings::default()), PipelineSettings::default());
    let bare = migrate::departure::caught_up(1, migrate::exposure::caught_up(1, bare));
    assert_eq!((decode(&former(before_1_0::LIGHTING_VERSION - 1, true)).print, decode(&former(before_1_0::LIGHTING_VERSION, false)).print), (kept.print, bare.print));
}

// MARK: - Fields

/// With its optional fields set, a document writes every field the table names, and no other.
#[test]
fn a_document_writes_every_field_the_table_names() {
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
        (s.manual_order, s.pass_of) = (Some(1.0), Some("p".into()));
        s.source_encoding = Some(oni_settings::SourceEncoding::Linear);
        s.rgb_passes = Some(RgbPasses { red: "r".into(), green: "g".into(), blue: "b".into(), registration: None });
        let (stops, black, white) = ([0.0; 3], [0.0; 3], [3.0; 3]);
        s.roll_reference = Some(oni_settings::frame::RollReference { id: Id::new(), stops, black, white, base: None, median: None });
        s.balance_method = Some("classic".into());
        (s.balance_filters, s.inverted_source, s.measured_gains) = (Some([0.0; 3]), true, Some([0.0; 3]));
        s.camera_channels = true;
        s.virtual_film = Some(oni_settings::VirtualFilm { stock: oni_settings::VirtualStock::of("kodak-portra-800"), exposure: 0.5 });
        s.print_paper = Some(oni_settings::PaperStock::of("kodak-endura-premier"));
        s.legacy_print = Some(Default::default());
        encoded(&s, VERSION)["settings"].as_object().unwrap().keys().cloned().collect()
    };
    let table = |fields: &[Field]| -> Vec<String> {
        let mut all: Vec<String> = fields.iter().map(|f| f.name().to_string()).collect();
        all.sort();
        all
    };
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(table(Field::ALL), sorted);
    // Twin: a table missing one field no longer covers the document.
    assert_ne!(table(&Field::ALL[1..]), sorted);
}

#[test]
fn the_rest_is_the_import_state_of_each_mode() {
    let negative = PipelineSettings::rest(ConversionMode::Negative, true);
    assert_eq!(negative.density_ceiling, density_axis::MOSAIC_REST);
    assert_eq!(negative.print.exposure, 0.0);
    assert!(negative.decoded_from_mosaic);
    assert_eq!(negative.levels, LevelsSet::neutral(ConversionMode::Negative, density_axis::MOSAIC_REST));
    let scan = PipelineSettings::rest(ConversionMode::Negative, false);
    assert_eq!(scan.density_ceiling, density_axis::base());
    let slide = PipelineSettings::rest(ConversionMode::Positive, true);
    assert_eq!(slide.density_ceiling, density_axis::base());
    assert_eq!(slide.print.exposure, 0.0);
    assert_eq!(slide.levels.red.black, Levels::resting_black(ConversionMode::Positive, density_axis::base()));
    let mono = PipelineSettings::rest(ConversionMode::Monochrome, false);
    assert_eq!(mono.print.exposure, 0.0);
    // Twin: the rest is not the bare struct, whose Lighting is unexposed.
    assert_ne!(negative, PipelineSettings::default());
    assert_ne!(PipelineSettings::rest(ConversionMode::Negative, false), PipelineSettings::rest(ConversionMode::Negative, true));
}

#[test]
fn a_former_balance_lands_in_the_filters_and_its_greys_render_as_they_did() {
    use oni_settings::curves::CurveSet;
    use oni_settings::migrate::lighting::{self as lighting, Curves, display, former_pixel, new_pixel};
    use oni_settings::tone::Tone;
    let mut raw = Object::new();
    let data = aged(&PipelineSettings::default(), before_1_0::LIGHTING_VERSION - 1, |s| {
        s.remove("print");
        s.insert("exposure".into(), json!(1.2));
        s.insert("toneBalance".into(), json!({"white": {"colour": [0.8, 0, -1.6]}, "shadows": {"colour": [0, 0.3, 0]}}));
        raw = s.clone();
    });
    let read = decode(&data);
    let f = lighting::former(&Value::Object(raw.clone())).expect("a former document");
    let started = std::time::Instant::now();
    let fit = lighting::fitted(&f, &CurveSet::default(), &CurveSet::default(), [1.0; 3]);
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
        let got = new_pixel([n(t); 3], &Print { exposure: FORMER_REST + GEAR * read.print.exposure, ..read.print }, read.density, [1.0; 3], false, &identity, &identity);
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
    let mut current = encoded(&PipelineSettings::default(), VERSION);
    current.insert("version".into(), json!(before_1_0::LIGHTING_VERSION - 1));
    current.insert("settings".into(), Value::Object(own));
    let kept = decode(&serde_json::to_vec(&Value::Object(current)).unwrap());
    assert_eq!(FORMER_REST + GEAR * kept.print.exposure, 0.5);
    assert_eq!((kept.print.red, kept.print.blue), (Tone::NEUTRAL, Tone::NEUTRAL));
    assert!(lighting::former(&json!({})).is_none());
}

#[test]
fn a_document_from_before_folds_its_scanner_look_and_one_since_carries_none() {
    let stored = PipelineSettings { density: 0.1, ..PipelineSettings::default() };
    let read = |version: i64| {
        decode(&aged(&stored, version, |s| {
            s.insert("look".into(), json!({"model": "frontier", "amount": 1.0}));
        }))
    };
    let older = read(chain::LOOK_FOLDED_VERSION - 1);
    assert!((older.density - 0.25).abs() < 1e-6, "{}", older.density);
    assert!(!older.manual_curves.red.is_neutral());
    // Twin: written since, the retired key is read as it stands, which is not at all.
    let newer = read(chain::LOOK_FOLDED_VERSION);
    assert_eq!(newer.density, 0.1);
    assert!(newer.manual_curves.is_neutral());
}

/// A sidecar still holding the retired `lensCorrected` and `rollReferenceDeclined` reads as one
/// without them, at its own version or this one, and is written back without them.
#[test]
fn the_retired_frame_flags_read_as_absent_and_are_written_no_more() {
    let placed = full();
    let flagged = |s: &mut Object| {
        s.insert("lensCorrected".into(), json!(true));
        s.insert("rollReferenceDeclined".into(), json!(true));
    };
    for version in [0, before_1_0::DENSITY_AXIS_VERSION, VERSION] {
        assert_eq!(decode(&aged(&placed, version, flagged)), decode(&aged(&placed, version, |_| {})), "version {version}");
    }
    // Twin: a key the settings read moves the frame, so the comparison sees a kept key.
    assert_ne!(decode(&aged(&placed, VERSION, |s| drop(s.insert("captureStops".into(), json!(0.75))))), decode(&aged(&placed, VERSION, |_| {})));
    let written = String::from_utf8(document::encode(&Document::of(decode(&aged(&placed, VERSION, flagged)))).unwrap()).unwrap();
    assert!(!written.contains("lensCorrected") && !written.contains("rollReferenceDeclined") && written.contains("captureStops"));
}
