//! Film grain: each grain's deviation and its size.

use serde::{Deserialize, Serialize};

/// Film grain on a digital photo's developed film: Amount, each grain's deviation as a share of its track's top, and
/// its size, a grain's width on the film in micrometres, so every resolution lays it alike on the picture.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Grain {
    pub amount: f32,
    pub size: f32,
}

impl Default for Grain {
    fn default() -> Self {
        Grain { amount: 0.0, size: Grain::SIZE_REST }
    }
}

impl Grain {
    pub const AMOUNT_RANGE: (f32, f32) = (0.0, 1.0);
    /// Film grain's 10 to 30 µm (Vitale, "Film Grain, Resolution and Fundamental Film Particles", 2010, table 1),
    /// to twice its coarsest.
    pub const SIZE_RANGE: (f32, f32) = (10.0, 60.0);
    /// The width a real Portra 800 scan's grain is matched at, every film's for now.
    pub const SIZE_REST: f32 = 30.0;
    /// The Grain a digital photo opens with (owner's call): the grain of a real Portra 800 scan.
    pub const DIGITAL_AMOUNT: f32 = 0.2545;
    /// The film frame's long side, in micrometres, which the scan's long side stands for: a 135 frame's.
    pub const FRAME_LONG: f64 = 36_000.0;

    /// Size in pixels of a scan whose long side is `long` pixels.
    pub fn pixels(size: f32, long: f64) -> f64 {
        f64::from(size) * long / Self::FRAME_LONG
    }

    /// The micrometres `pixels` of a scan whose long side is `long` pixels stand for.
    pub fn microns(pixels: f64, long: f64) -> f32 {
        (pixels * Self::FRAME_LONG / long) as f32
    }
}
