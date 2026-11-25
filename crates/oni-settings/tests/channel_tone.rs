//! Each channel's own mids and contrast in the sidecar: a document from
//! before them reads them at rest, one of this format keeps them, a frame at rest writes no channel key.

use oni_settings::document::VERSION;
use oni_settings::migrate::{self, chain};
use oni_settings::tone::Print;
use oni_settings::{ConversionMode, Document, PipelineSettings};
use serde_json::json;

/// The `oni-frame` document of `settings` stamped `version`.
fn stored(settings: &PipelineSettings, version: i64) -> Vec<u8> {
    let mut object = serde_json::to_value(migrate::departure::stored_at(&Document::of(settings.clone()), version)).unwrap();
    object["version"] = json!(version);
    serde_json::to_vec(&object).unwrap()
}

#[test]
fn a_document_from_before_the_channels_tone_reads_it_at_rest_and_a_current_one_keeps_it() {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.print.channel_white = [0.0, 0.0625, 0.0];
    let older = stored(&s, chain::CHANNEL_TONE_VERSION - 1);
    assert_eq!(migrate::standing(&older), migrate::Standing::Older(chain::CHANNEL_TONE_VERSION - 1));
    let read = migrate::document(&older).unwrap().document.settings;
    assert_eq!((read.print.channel_mid, read.print.channel_contrast), ([0.0; 3], [0.0; 3]));
    assert_eq!(read, migrate::departure::read_back(chain::CHANNEL_TONE_VERSION - 1, s.clone()), "the older document reads whole");
    // Twin: a document of this format keeps a channel's own tone to the bit.
    s.print.channel_mid = [0.0, -0.1373, 0.0];
    s.print.channel_contrast = [0.0625, 0.0, -0.25];
    let current = stored(&s, VERSION);
    assert_eq!(migrate::standing(&current), migrate::Standing::Current);
    let kept = migrate::document(&current).unwrap().document.settings;
    assert_eq!((kept.print.channel_mid, kept.print.channel_contrast), ([0.0, -0.1373, 0.0], [0.0625, 0.0, -0.25]));
}

#[test]
fn a_frame_at_rest_writes_no_channel_tone() {
    let rest = PipelineSettings::rest(ConversionMode::Negative, false);
    let text = oni_settings::document::encode_settings(&rest).unwrap();
    assert!(!text.contains("channelMid") && !text.contains("channelContrast"));
    // Twin: one channel's contrast a hundredth off rest is written.
    let mut blue = rest.clone();
    blue.print.channel_contrast[2] = 0.01;
    assert!(oni_settings::document::encode_settings(&blue).unwrap().contains("channelContrast"));
}

#[test]
fn a_channels_tone_past_its_track_lands_on_its_end() {
    let print = |mid: f32, contrast: f32| -> Print {
        serde_json::from_value(json!({ "channelMid": [mid, 0.0, -mid], "channelContrast": [0.0, contrast, -contrast] })).unwrap()
    };
    let far = print(0.9, 2.0);
    assert_eq!(far.channel_mid, [Print::ZONE_RANGE.1, 0.0, Print::ZONE_RANGE.0]);
    assert_eq!(far.channel_contrast, [0.0, Print::CONTRAST_RANGE.1, Print::CONTRAST_RANGE.0]);
    // Twin: a value inside its track is kept as it was written.
    assert_eq!(print(0.24, 0.49).channel_mid[0], 0.24);
    assert_ne!(print(0.24, 0.49).channel_mid[0], Print::ZONE_RANGE.1);
}
