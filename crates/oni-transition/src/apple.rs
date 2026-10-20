//! Apple's RAW decode as the Swift app ran it, the reference the earlier settings were measured on.
//! A check hands a stand-in that needs no Core Image.

use crate::measure::{Grid, Plane};
use std::path::Path;

/// Apple's decode of a RAW, averaged over `measure::BLOCK`-pixel squares from its top-left pixel.
pub trait AppleDecode: Send + Sync {
    fn blocks(&self, raw: &Path) -> Result<Grid, String>;
}

/// LibRaw's own decode gained by fixed stops per channel: what the Swift app would have read on a
/// camera whose shift is known, so a check runs the whole transition on any machine.
pub struct Simulated {
    pub stops: [f64; 3],
}

impl AppleDecode for Simulated {
    fn blocks(&self, raw: &Path) -> Result<Grid, String> {
        let decoded = oni_decode::decode(raw, 1.0).map_err(|e| format!("decode {}", e.0))?;
        let gains = self.stops.map(|s| s.exp2() as f32);
        let pixels: Vec<[f32; 4]> = decoded.pixels.iter().map(|p| [p[0] * gains[0], p[1] * gains[1], p[2] * gains[2], 1.0]).collect();
        Ok(Grid::of(Plane { width: decoded.width, height: decoded.height, pixels: &pixels }))
    }
}

#[cfg(target_os = "macos")]
mod core_image;
#[cfg(target_os = "macos")]
pub use core_image::CoreImage;
