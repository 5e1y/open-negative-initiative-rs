//! The help cards' demos: each a coded scene of the design system's real components over a frame
//! rendered by the app's engine, filmed headless at 20 frames a second and written as a small GIF.

pub mod cursor;
pub mod demos;
pub mod encode;
pub mod film;
pub mod photo;
pub mod plot;
pub mod sheet;
pub mod stage;
pub mod track;

use std::path::{Path, PathBuf};

/// Where the app takes its demos from, `include_bytes!`'d by `oni_app::demos`.
pub fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../oni-app/resources/demos")
}
