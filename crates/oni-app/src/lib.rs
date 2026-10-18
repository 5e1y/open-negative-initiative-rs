//! The desktop shell of Open Negative Initiative: the window, the bar, the panel, the canvas and the
//! gallery, drawn by Slint and driven by the editing model of `oni-core`.

pub mod app;
pub mod bench;
pub mod display;
pub mod export;
pub mod fields;
pub mod flat_fields;
pub mod framing;
pub mod gallery;
pub mod heal;
pub mod library;
pub mod motion;
pub mod patches;
pub mod phone;
pub mod preferences;
pub mod proof;
pub mod render;
pub mod store;
pub mod strict;
pub mod system;
pub mod timing;
pub mod tokens;
pub mod track;
pub mod viewport;

use std::path::PathBuf;

slint::include_modules!();

/// The Swift app's `Sources/OpenNegative`, which the token generator reads: `ONI_SWIFT_APP`, else
/// a checkout beside this workspace.
pub fn swift_sources() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("ONI_SWIFT_APP") {
        return Some(PathBuf::from(dir));
    }
    let beside = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../open-negative-initiative/Sources/OpenNegative");
    beside.is_dir().then_some(beside)
}
