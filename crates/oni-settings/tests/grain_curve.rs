//! The grain's strength by tone in the sidecar: a document from before it reads it at rest, one of this
//! format keeps a moved point, a frame at rest writes no curve.

use oni_settings::blocks::GrainCurve;
use oni_settings::cg::Point;
use oni_settings::document::VERSION;
use oni_settings::migrate::{self, chain};
use oni_settings::{ConversionMode, Document, PipelineSettings};
use serde_json::json;

/// The `oni-frame` document of `settings` stamped `version`.
fn stored(settings: &PipelineSettings, version: i64) -> Vec<u8> {
    let mut object = serde_json::to_value(Document::of(settings.clone())).unwrap();
    object["version"] = json!(version);
    serde_json::to_vec(&object).unwrap()
}

#[test]
fn a_document_from_before_the_grain_curve_reads_it_at_rest_and_a_current_one_keeps_it() {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true);
    s.grain.amount = 0.4;
    let older = stored(&s, chain::GRAIN_CURVE_VERSION - 1);
    assert!(!String::from_utf8_lossy(&older).contains("\"curve\""));
    assert_eq!(migrate::standing(&older), migrate::Standing::Older(chain::GRAIN_CURVE_VERSION - 1));
    let read = migrate::document(&older).unwrap().document.settings;
    assert_eq!(read.grain.curve, GrainCurve::REST);
    assert_eq!(read, s, "the older document reads whole");
    // Twin: a document of this format keeps a moved point to the bit.
    s.grain.curve.move_point(3, Point::new(0.9137, 0.0625));
    let current = stored(&s, VERSION);
    assert_eq!(migrate::standing(&current), migrate::Standing::Current);
    let kept = migrate::document(&current).unwrap().document.settings;
    assert_eq!(kept.grain.curve.points[3], Point::new(0.9137, 0.0625));
    assert_ne!(kept.grain.curve, GrainCurve::REST);
}

#[test]
fn a_frame_at_rest_writes_no_grain_curve() {
    let mut rest = PipelineSettings::rest(ConversionMode::Digital, true);
    rest.grain.amount = 0.7;
    let text = oni_settings::document::encode_settings(&rest).unwrap();
    assert!(text.contains("\"grain\"") && !text.contains("\"curve\""));
    // Twin: one point a hundredth off rest is written.
    let mut moved = rest.clone();
    moved.grain.curve.move_point(0, Point::new(0.0, 0.36));
    assert!(oni_settings::document::encode_settings(&moved).unwrap().contains("\"curve\""));
}
