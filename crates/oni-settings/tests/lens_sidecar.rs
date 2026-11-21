//! A digital photo's lens corrections in its sidecar: a file from before them reads none, a corrected
//! one keeps all four values, and a paste carries them as it carries the look.

use oni_settings::document::{self, VERSION};
use oni_settings::lens::LensCorrection;
use oni_settings::migrate::{self, chain};
use oni_settings::settings::look_settings;
use oni_settings::{ConversionMode, Document, PipelineSettings};
use serde_json::{Value, json};

fn corrected() -> LensCorrection {
    LensCorrection { distortion: 31.5, red: -12.25, blue: 7.0, vignetting: 44.0 }
}

/// `settings` written at `version`, its `lensCorrection` key dropped when `bare`.
fn stored(settings: &PipelineSettings, version: i64, bare: bool) -> Vec<u8> {
    let mut object = serde_json::to_value(Document::of(settings.clone())).unwrap();
    object["version"] = json!(version);
    if bare && let Some(Value::Object(s)) = object.get_mut("settings") {
        s.remove("lensCorrection");
    }
    serde_json::to_vec(&object).unwrap()
}

#[test]
fn a_sidecar_from_before_the_lens_reads_none_and_a_corrected_one_keeps_its_four_values() {
    let digital = PipelineSettings::rest(ConversionMode::Digital, true);
    // Its print rested then at no exposure, which today's rest stands on.
    let then = PipelineSettings { print: oni_settings::tone::Print { exposure: 0.0, ..digital.print }, ..digital.clone() };
    let before = migrate::document(&stored(&then, chain::LENS_CORRECTION_VERSION - 2, true)).unwrap();
    assert_eq!(before.document.settings.lens_correction, LensCorrection::default());
    assert_eq!(before.document.settings, digital);
    let lens = PipelineSettings { lens_correction: corrected(), ..digital.clone() };
    let text = document::encode(&Document::of(lens.clone())).unwrap();
    assert!(String::from_utf8_lossy(&text).contains("\"lensCorrection\""));
    assert_eq!(document::decode(&text).unwrap().settings.lens_correction, corrected());
    // Twin: the key dropped from today's file reads the lens at rest, so the round trip above saw it.
    assert_ne!(migrate::document(&stored(&lens, VERSION, true)).unwrap().document.settings.lens_correction, corrected());
}

#[test]
fn a_paste_carries_the_lens_as_it_carries_the_look() {
    let donor = PipelineSettings { lens_correction: corrected(), ..PipelineSettings::rest(ConversionMode::Digital, true) };
    let target = PipelineSettings::rest(ConversionMode::Digital, false);
    assert!(look_settings::FIELDS.contains(&"lensCorrection"));
    assert_eq!(look_settings::copied(&donor, &target).lens_correction, corrected());
    // Twin: a roll's own fields stay the target's.
    assert!(!look_settings::copied(&donor, &target).decoded_from_mosaic);
}

#[test]
fn a_film_mode_keeps_its_lens_unrendered() {
    let size = (6000.0, 4000.0);
    for mode in [ConversionMode::Negative, ConversionMode::Positive, ConversionMode::Monochrome] {
        let s = PipelineSettings { lens_correction: corrected(), ..PipelineSettings::rest(mode, true) };
        assert_eq!((s.lens(size), s.optical_vignetting()), (None, None), "{mode:?}");
        assert_eq!(s.placing(size), s.geometry.placing(size), "{mode:?}");
    }
    // Twin: the same values on a digital photo map and vignette.
    let digital = PipelineSettings { lens_correction: corrected(), ..PipelineSettings::rest(ConversionMode::Digital, true) };
    assert!(digital.lens(size).is_some() && digital.optical_vignetting() == Some(-0.88));
    assert_ne!(digital.placing(size), digital.geometry.placing(size));
}
