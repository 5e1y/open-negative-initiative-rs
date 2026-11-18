//! Each channel's own white and black points in the sidecar: a document from before them reads them at
//! zero and is brought, one of this format keeps them, and a frame at rest writes no key for them.

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
fn a_document_from_before_the_channels_points_reads_them_at_zero_and_a_current_one_keeps_them() {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.print.white_point = 0.125;
    s.print.exposure = 0.3137;
    let older = stored(&s, chain::CHANNEL_POINTS_VERSION - 1);
    assert_eq!(migrate::standing(&older), migrate::Standing::Older(chain::CHANNEL_POINTS_VERSION - 1));
    let read = migrate::document(&older).unwrap().document.settings;
    assert_eq!((read.print.channel_white, read.print.channel_black), ([0.0; 3], [0.0; 3]));
    assert_eq!(read, s, "the older document reads whole");
    // Twin: a document of this format keeps a channel's own points to the bit.
    s.print.channel_black = [0.0, 0.0625, 0.0];
    s.print.channel_white = [-0.1373, 0.0, 0.0];
    let current = stored(&s, VERSION);
    assert_eq!(migrate::standing(&current), migrate::Standing::Current);
    let kept = migrate::document(&current).unwrap().document.settings;
    assert_eq!((kept.print.channel_white, kept.print.channel_black), ([-0.1373, 0.0, 0.0], [0.0, 0.0625, 0.0]));
}

#[test]
fn a_frame_at_rest_writes_no_channel_point() {
    let rest = PipelineSettings::rest(ConversionMode::Positive, false);
    let text = oni_settings::document::encode_settings(&rest).unwrap();
    assert!(!text.contains("channelWhite") && !text.contains("channelBlack"));
    // Twin: one channel's point a hundredth off rest is written.
    let mut green = rest.clone();
    green.print.channel_white[1] = 0.01;
    assert!(oni_settings::document::encode_settings(&green).unwrap().contains("channelWhite"));
}
