//! The desktop shell of Open Negative Initiative: the window, the bar, the panel, the canvas and the
//! gallery, drawn by Slint and driven by the editing model of `oni-core`.

pub mod app;
pub mod bench;
pub mod chrome;
pub mod decode_gate;
pub mod display;
pub mod editors;
pub mod export;
pub mod fields;
pub mod flat_fields;
pub mod framing;
pub mod gallery;
pub mod heal;
pub mod histogram;
pub mod library;
pub mod loupe;
pub mod measuring;
pub mod motion;
pub mod organise;
pub mod passes;
pub mod pasteboard;
pub mod patches;
pub mod phone;
pub mod picking;
pub mod preferences;
pub mod splash;
pub mod proof;
pub mod report;
pub mod render;
pub mod roll_previews;
pub mod scans;
pub mod sidebar;
pub mod slider_track;
pub mod stocks;
pub mod store;
pub mod strict;
pub mod system;
pub mod tabs;
pub mod thumbnails;
pub mod timing;
pub mod tokens;
pub mod track;
pub mod tree_pick;
pub mod updates;
pub mod viewport;
pub mod web;
pub mod watchdog;
pub mod wrapped;

use std::path::PathBuf;

pub use oni_ui::*;

/// The Swift app's `Sources/OpenNegative`, which the token generator reads: `ONI_SWIFT_APP`, else
/// a checkout beside this workspace.
pub fn swift_sources() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("ONI_SWIFT_APP") {
        return Some(PathBuf::from(dir));
    }
    let beside = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../open-negative-initiative/Sources/OpenNegative");
    beside.is_dir().then_some(beside)
}
