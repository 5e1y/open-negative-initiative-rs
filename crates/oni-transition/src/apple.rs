//! Apple's RAW decode as the Swift app ran it, the reference the earlier settings were measured on.
//! A check hands a stand-in that needs no Core Image.

use crate::measure::{Grid, M3, Plane, apply};
use std::path::Path;

/// Apple's decode of a RAW, averaged over `measure::BLOCK`-pixel squares from its top-left pixel.
pub trait AppleDecode: Send + Sync {
    fn blocks(&self, raw: &Path) -> Result<Grid, String>;
}

/// LibRaw's own decode through a fixed 3x3: what the Swift app would have read on a camera whose
/// matrix is known, so a check runs the whole transition on any machine.
pub struct Simulated {
    pub matrix: M3,
}

impl AppleDecode for Simulated {
    fn blocks(&self, raw: &Path) -> Result<Grid, String> {
        let decoded = oni_decode::decode_native(raw, 1.0).map_err(|e| format!("decode {}", e.0))?;
        let pixels: Vec<[f32; 4]> = decoded
            .pixels
            .iter()
            .map(|p| {
                let v = apply(&self.matrix, [0, 1, 2].map(|c| f64::from(p[c])));
                [v[0] as f32, v[1] as f32, v[2] as f32, 1.0]
            })
            .collect();
        Ok(Grid::of(Plane { width: decoded.width, height: decoded.height, pixels: &pixels }))
    }
}

#[cfg(target_os = "macos")]
mod core_image;
#[cfg(target_os = "macos")]
pub use core_image::CoreImage;
