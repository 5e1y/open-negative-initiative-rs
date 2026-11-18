//! The halation's arguments: the highlights' light past a threshold, blurred over a share of the frame
//! and laid back on the scene before the print, its red layer's most.

use crate::stages::{self, Before};
use oni_graph::{Image, Rect};
use oni_render::Halation;
use oni_settings::PipelineSettings;

/// The narrowest glow's deviation, a share of the scan's width: six pixels on a scan 6000 wide.
pub const HALATION_NARROWEST: f64 = 0.001;
/// How many times Size's top widens the narrowest glow.
pub const HALATION_WIDEST: f64 = 4.0;

/// The glow's deviation in this render's pixels, on a scan of extent `scan`: a share of its width
/// whatever its scale.
pub fn deviation(s: &PipelineSettings, scan: Rect) -> f64 {
    f64::from(scan.w) * HALATION_NARROWEST * HALATION_WIDEST.powf(f64::from(s.halation.size))
}

/// `image` with the halation of `s`, on a scan of extent `scan`; itself at rest, with no stage.
pub fn halated(image: Image, s: &PipelineSettings, scan: Rect) -> Image {
    if s.halation.amount == 0.0 {
        return image;
    }
    let glow = stages::blurred_of(image.clone(), deviation(s, scan), Before::Halation);
    image.then_reading(stages::halation(&Halation { amount: s.halation.amount }), &[glow])
}
