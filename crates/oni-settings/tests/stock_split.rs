//! A frame's measured dyes across the split of the lines into their stocks: a frame of format 39 reads unmoved,
//! every set reads by its own key; each verdict with a twin.

use oni_settings::document::{self, VERSION};
use oni_settings::migrate::{self, chain};
use oni_settings::{ConversionMode, Document, PipelineSettings};
use serde_json::{Value, json};

/// A frame of format 39 reads its dyes and dose unmoved, every set of 40 by its own key; a build of 39 refused a
/// set of a split stock, which is why the format moved; a key no file defines now reads as stored.
#[test]
fn a_frame_of_39_reads_its_dyes_unmoved_and_a_split_stocks_set_is_of_40() {
    use oni_settings::DyeSet;
    assert!(VERSION >= chain::STOCK_SPLIT_VERSION);
    let stamped = |set: DyeSet, version: i64| {
        let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
        (s.dye_set, s.separation) = (set, 0.8);
        let mut object = serde_json::to_value(Document::of(s)).unwrap();
        object["version"] = json!(version);
        serde_json::to_vec(&object).unwrap()
    };
    let read = |bytes: &[u8]| migrate::converted(bytes, &migrate::Facts::bare(true)).unwrap().document;
    for set in [DyeSet::of("kodak-portra-400"), DyeSet::of("fujifilm-c200"), DyeSet::of("generic-c41")] {
        let bytes = stamped(set, chain::STOCK_DYES_VERSION);
        assert_eq!(migrate::standing(&bytes), migrate::Standing::Older(chain::STOCK_DYES_VERSION));
        let doc = read(&bytes);
        assert_eq!((doc.version, doc.settings.dye_set, doc.settings.separation), (VERSION, set, 0.8), "{set:?}");
    }
    for set in DyeSet::all() {
        assert_eq!(read(&stamped(set, VERSION)).settings.dye_set, set, "{set:?}");
    }
    // Twin: a set's key no file defines is kept as stored, never read as another set.
    let mut unknown: Value = serde_json::from_slice(&stamped(DyeSet::of("kodak-ektar-100"), VERSION)).unwrap();
    unknown["settings"]["dyeSet"] = json!("kodak-ektar-200");
    let kept = document::decode(&serde_json::to_vec(&unknown).unwrap()).expect("it reads").settings.dye_set;
    assert!(kept.key() == "kodak-ektar-200" && !DyeSet::all().contains(&kept));
}
