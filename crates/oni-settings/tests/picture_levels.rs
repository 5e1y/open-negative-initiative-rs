//! A digital photo's own levels: each window's transfer on the picture, and the sidecar keeping them; a
//! document from before them reads them at rest, and a frame at rest writes no key for them.

use oni_settings::document::VERSION;
use oni_settings::levels::{Levels, PictureLevels};
use oni_settings::migrate::{self, chain};
use oni_settings::migrate::density::apply_window;
use oni_settings::{ConversionMode, Document, PipelineSettings};
use serde_json::json;

fn stored(settings: &PipelineSettings, version: i64) -> Vec<u8> {
    let mut object = serde_json::to_value(Document::of(settings.clone())).unwrap();
    object["version"] = json!(version);
    serde_json::to_vec(&object).unwrap()
}

#[test]
fn a_window_at_rest_is_the_identity_and_a_moved_one_goes_on_past_its_white_at_its_slope() {
    let rest = Levels::default();
    for i in 0..=250 {
        let x = -0.5 + f64::from(i) * 0.01;
        assert_eq!(rest.transfer(x), x, "at rest, at {x}");
    }
    assert_eq!(rest.end_slope(), 1.0);
    let moved = Levels { black: 0.08, white: 0.7, mid: 0.4, shadows: 0.3, highlights: 0.65 };
    let w = f64::from(moved.white);
    let h = 1e-7;
    let below = moved.transfer(w - h);
    assert!((moved.transfer(w) - 1.0).abs() < 1e-12 && (below - 1.0).abs() < 1e-5, "continuous at the white: {below}");
    let left = (moved.transfer(w) - moved.transfer(w - h)) / h;
    let right = (moved.transfer(w + 0.3) - moved.transfer(w)) / 0.3;
    assert!((left - moved.end_slope()).abs() < 1e-5 && (right - moved.end_slope()).abs() < 1e-9, "{left} {right} {}", moved.end_slope());
    assert_eq!((moved.transfer(0.05), moved.transfer(-0.2)), (0.0, -0.2), "nothing under the black, a negative left");
    // Inside the window it is the film window's own curve, read on the picture.
    let t = 0.4;
    assert!((moved.transfer(t) - f64::from(apply_window(t as f32, &moved))).abs() < 1e-6);
    // Twin: the film window's continuation past its white, `t − u`, is not the slope reached.
    let film = f64::from(apply_window(1.0, &moved));
    assert!((film - moved.transfer(1.0)).abs() > 0.05, "{film} {}", moved.transfer(1.0));
}

#[test]
fn a_document_from_before_the_picture_levels_reads_them_at_rest_and_a_current_one_keeps_them() {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
    s.print.exposure = 0.25;
    let older = stored(&s, chain::PICTURE_LEVELS_VERSION - 1);
    assert_eq!(migrate::standing(&older), migrate::Standing::Older(chain::PICTURE_LEVELS_VERSION - 1));
    let read = migrate::document(&older).unwrap().document.settings;
    assert!(read.picture_levels.is_neutral());
    assert_eq!(read, s, "the older document reads whole");
    // Twin: a document of this format keeps a moved window to the bit.
    s.picture_levels.luma = Levels { black: 0.0625, white: 0.8125, mid: 0.4375, ..Levels::default() };
    let current = stored(&s, VERSION);
    assert_eq!(migrate::standing(&current), migrate::Standing::Current);
    assert_eq!(migrate::document(&current).unwrap().document.settings.picture_levels, s.picture_levels);
}

#[test]
fn a_frame_at_rest_writes_no_picture_levels() {
    let rest = PipelineSettings::rest(ConversionMode::Digital, false);
    assert!(!oni_settings::document::encode_settings(&rest).unwrap().contains("pictureLevels"));
    // Twin: one handle off rest is written.
    let mut blue = rest.clone();
    blue.picture_levels = PictureLevels { blue: Levels { highlights: 0.7, ..Levels::default() }, ..PictureLevels::default() };
    assert!(oni_settings::document::encode_settings(&blue).unwrap().contains("pictureLevels"));
}
