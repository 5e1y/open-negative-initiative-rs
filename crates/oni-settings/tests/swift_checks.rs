//! The Swift app's self-checks on the models and the sidecar, each on its own adversarial values and
//! with its twin, a variant built to fail when the property it guards is broken.

use oni_settings::axis::{density_axis, pipeline};
use oni_settings::cg::{Point, Rect};
use oni_settings::codec;
use oni_settings::frame::{Correction, FlatField, PassOrder};
use oni_settings::levels::{Levels, LevelsChannel, LevelsSet};
use oni_settings::sidecar::{self, DraftBase, WriteOptions};
use oni_settings::tone::color_density;
use oni_settings::{ConversionMode, ExifMetadata, Id, PipelineSettings, RollDraft};
use serde_json::Value;
use std::path::{Path, PathBuf};

type V3 = [f32; 3];

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("oni-settings-swift-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// MARK: - ConversionMode.selfCheck

#[test]
fn monochrome_inverts_and_rests_like_a_negative_and_a_positive_on_its_own() {
    let mono = ConversionMode::Monochrome;
    assert!(mono.invert_flag() == 1.0 && mono.inverts_gain_sense());
    assert_eq!(Levels::resting_mid(mono), Levels::resting_mid(ConversionMode::Negative));
    // Twin: a positive neither inverts nor flips the gains.
    assert!(ConversionMode::Positive.invert_flag() == 0.0 && !ConversionMode::Positive.inverts_gain_sense());

    let base = density_axis::base();
    assert_eq!(LevelsSet::neutral(ConversionMode::Negative, base), LevelsSet::default());
    assert_eq!(LevelsSet::neutral(mono, base), LevelsSet::default());
    assert_eq!(LevelsSet::resting(LevelsChannel::Red, ConversionMode::Negative, base).black, 0.0);
    // Twin: a positive rests its blacks elsewhere.
    assert_ne!(LevelsSet::neutral(ConversionMode::Positive, base), LevelsSet::default());
}

// MARK: - DensityAxis.selfCheck

#[test]
fn the_density_axis_rests_on_the_frozen_pedestal_and_its_guard_on_the_white() {
    const FROZEN_PEDESTAL: f32 = 0.008;
    let base = density_axis::base();
    assert_eq!(density_axis::pedestal(base), FROZEN_PEDESTAL, "{:e}", density_axis::pedestal(base));
    // Twin: the same ceiling quoted to three decimals refuses.
    assert_ne!(density_axis::pedestal(2.097), FROZEN_PEDESTAL);

    assert!((base + density_axis::guard_offset() - pipeline::DMAX).abs() < 1e-6);
    // Twin: at the range's top the guard's density stands past that white.
    let top = *density_axis::range().end();
    assert!(-density_axis::log_guard(top).log10() - pipeline::DMAX > 0.1);

    let range = density_axis::range();
    let landing = |stops: f32| {
        (0..11)
            .filter(|&step| {
                let c = range.start() + (range.end() - range.start()) * step as f32 / 10.0;
                density_axis::pedestal(c) * stops.exp2() == density_axis::log_guard(c)
            })
            .count()
    };
    assert_eq!(landing(-3.0), 11, "-3 stops lands the pedestal exactly on the guard everywhere");
    // Twin: a tenth of a stop away lands nowhere, so the identity names one gain.
    assert_eq!(landing(-2.9), 0);
}

// MARK: - ColorDensity.selfCheck

#[test]
fn colour_density_keeps_a_neutral_and_raises_a_ratio_to_its_power() {
    let travel = 1.0;
    let mut worst = 0.0f32;
    for step in 1..=1000 {
        let g = step as f32 / 1000.0;
        let out = color_density::rendered([g; 3], travel);
        let drift = out.iter().map(|&v| (v - g).abs()).fold(0.0, f32::max) / g;
        worst = worst.max(drift);
    }
    assert!(worst < 1e-4, "a neutral drifts {worst:.2e}");

    let colour: V3 = [0.55, 0.22, 0.16];
    let done = color_density::rendered(colour, travel);
    let wanted = (colour[0] / colour[2]).powf(color_density::factor(travel));
    assert!((done[0] / done[2] - wanted).abs() < 1e-3 * wanted);

    let y0 = pipeline::luma(colour);
    let gain = pipeline::luma(done) / pipeline::luma(color_density::rendered([y0; 3], travel)) - 1.0;
    assert!(gain > 0.05, "a colour comes back only {:+.1} % brighter than its grey", gain * 100.0);
    // Twin: the opposite travel closes the gap, so the line above is not a mere brightening.
    let closed =
        pipeline::luma(color_density::rendered(colour, -travel)) / pipeline::luma(color_density::rendered([y0; 3], -travel)) - 1.0;
    assert!(closed < 0.0, "the opposite travel leaves {:+.1} %", closed * 100.0);

    let ordinary: [V3; 12] = [
        [0.45, 0.45, 0.45],
        [0.62, 0.46, 0.38],
        [0.55, 0.22, 0.16],
        [0.24, 0.42, 0.18],
        [0.30, 0.44, 0.68],
        [0.66, 0.58, 0.30],
        [0.82, 0.74, 0.66],
        [0.14, 0.16, 0.20],
        [0.71, 0.35, 0.28],
        [0.38, 0.52, 0.55],
        [0.90, 0.86, 0.80],
        [0.20, 0.24, 0.16],
    ];
    let peak = ordinary.iter().map(|&c| color_density::rendered(c, travel).into_iter().fold(0.0, f32::max)).fold(0.0, f32::max);
    assert!(peak <= 1.1, "ordinary colours peak at {peak:.3}");
    // Twin: a near-primary does leave the scale.
    let primary = color_density::rendered([0.95, 0.05, 0.05], travel).into_iter().fold(0.0, f32::max);
    assert!(primary > 1.5, "a near-primary reaches only {primary:.3}");
    assert_eq!(color_density::FULL_POWER, 2.0);
}

// MARK: - RollDraft.selfCheck

fn draft() -> RollDraft {
    RollDraft {
        id: Id::new(),
        step: "flags".into(),
        film: ConversionMode::Negative,
        capture: "camera".into(),
        pass_order: Some(PassOrder::Bgr),
        flat_field: Some(FlatField::default()),
        base: Some(DraftBase { path: "/r/1.RW2".into(), x: 0.1, y: 0.2, radius: 4.0 }),
        window: Some(Rect::new(0.1, 0.1, 0.8, 0.8)),
        ticked: true,
        flagged: true,
    }
}

#[test]
fn a_roll_draft_rides_its_frames_sidecar_and_a_clear_takes_it_alone() {
    let draft = draft();
    let settings = PipelineSettings::rest(ConversionMode::Negative, true);
    let source = codec::Source { mosaic: &|| true, plain: &|| false };
    let written = codec::encode(&settings, None, None, None, Some(draft.clone()), None).unwrap();
    let read = codec::decode_with(written.as_bytes(), &source, false).unwrap();
    assert_eq!(read.roll_draft.as_ref(), Some(&draft));
    assert_eq!(read.settings, settings);
    // Twin: a document written with no draft reads none.
    let bare = codec::decode_with(codec::encode_settings(&settings).unwrap().as_bytes(), &source, false).unwrap();
    assert_eq!(bare.roll_draft, None);

    let folder = scratch("roll-draft");
    let photo = folder.join("frame.tif");
    std::fs::write(&photo, [0]).unwrap();
    sidecar::set_roll_draft(None, &photo, false).unwrap();
    assert!(!sidecar::url_for(&photo).exists(), "clearing a draft on a frame never touched writes no sidecar");
    sidecar::set_roll_draft(Some(draft.clone()), &photo, false).unwrap();
    let mut moved = settings.clone();
    moved.print.exposure += 0.5;
    sidecar::write(&moved, &photo, WriteOptions::default()).unwrap();
    assert_eq!(sidecar::read(&photo, false, true).unwrap().roll_draft, Some(draft), "a settings write keeps the draft");
    sidecar::set_roll_draft(None, &photo, false).unwrap();
    let cleared = sidecar::read(&photo, false, true).unwrap();
    assert!(cleared.roll_draft.is_none() && cleared.settings.print.exposure == moved.print.exposure);
}

// MARK: - Sidecar.exifChecks

#[test]
fn the_frames_exif_rides_its_sidecar_and_an_empty_box_is_absent() {
    let folder = scratch("sidecar-exif");
    let original = folder.join("frame.RW2");
    std::fs::write(&original, b"raw").unwrap();
    let typed = ExifMetadata {
        camera: "Nikon FM2".into(),
        film: "Kodak Portra 400".into(),
        date: "2024:05:01 10:00:00".into(),
        ..ExifMetadata::default()
    };
    let mut edited = PipelineSettings::default();
    edited.gains.stops = [0.5, 0.0, 0.0];
    sidecar::write(&edited, &original, WriteOptions::default()).unwrap();
    assert_eq!(sidecar::read(&original, true, true).unwrap().exif, None);

    sidecar::set_exif(typed.clone(), &original, true).unwrap();
    let loaded = sidecar::read(&original, true, true).unwrap();
    assert!(loaded.exif.as_ref() == Some(&typed) && loaded.settings == edited, "setExif touches the EXIF alone");

    let mut later = edited.clone();
    later.gains.stops = [1.0, 0.0, 0.0];
    sidecar::write(&later, &original, WriteOptions::default()).unwrap();
    assert_eq!(sidecar::read(&original, true, true).unwrap().exif, Some(typed), "a settings write keeps the EXIF");
    // Twin: the encoder handed nothing drops it.
    assert_eq!(codec::decode(codec::encode_settings(&later).unwrap().as_bytes()).unwrap().exif, None);

    sidecar::set_exif(ExifMetadata::default(), &original, true).unwrap();
    let text = std::fs::read_to_string(sidecar::url_for(&original)).unwrap();
    assert!(sidecar::read(&original, true, true).unwrap().exif.is_none() && !text.contains("\"exif\""));
}

// MARK: - Correction.selfCheck

#[test]
fn a_correction_round_trips_and_its_absent_keys_read_their_defaults() {
    let id = Id::parse("11111111-2222-3333-4444-555555555555").unwrap();
    let full = Correction { id, stroke: vec![Point::new(0.12, 0.34), Point::new(0.56, 0.78)], radius: 9.5 };
    let data = serde_json::to_string(&full).unwrap();
    assert_eq!(serde_json::from_str::<Correction>(&data).unwrap(), full);

    let mut object: Value = serde_json::from_str(&data).unwrap();
    object.as_object_mut().unwrap().remove("radius");
    let read: Correction = serde_json::from_value(object).unwrap();
    assert!(read.radius == Correction::DEFAULT_RADIUS && read.id == full.id && read.stroke == full.stroke);
    // Twin: a radius actually written reads back as written, so the default above was read.
    assert!(full.radius == 9.5 && full.radius != Correction::DEFAULT_RADIUS);
    assert_eq!(Correction::DEFAULT_RADIUS, 12.0);

    let settings = PipelineSettings { corrections: vec![full.clone()], ..PipelineSettings::default() };
    let decoded = codec::decode(codec::encode_settings(&settings).unwrap().as_bytes()).unwrap();
    assert_eq!(decoded.settings.corrections, vec![full]);

    let mut bare: Value = serde_json::from_str(&codec::encode_settings(&PipelineSettings::default()).unwrap()).unwrap();
    assert!(bare["settings"].as_object_mut().unwrap().remove("corrections").is_some());
    let read = codec::decode(serde_json::to_string(&bare).unwrap().as_bytes()).unwrap();
    assert!(read.settings.corrections.is_empty());
}
