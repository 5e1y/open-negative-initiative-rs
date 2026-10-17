//! The ICC conversion: working space to XYZ, our own black point compensation, XYZ to the
//! profile in relative colorimetric, then the clip; and the profile read back to working light.

use std::sync::Arc;

use moxcms::{ColorProfile, InterpolationMethod, Layout, RenderingIntent, TransformF32Executor, TransformOptions};

use crate::pcs::{self, PCS_WHITE};
use crate::profile::{self, OutputProfile};
use crate::icc_lut::Table;
use crate::working::{PCS_PROFILE, PCS_TO_WORKING, WORKING_TO_PCS};

/// Relative colorimetric: the rest is clipped, so the output clip stays the only compression.
pub fn relative() -> TransformOptions {
    TransformOptions {
        rendering_intent: RenderingIntent::RelativeColorimetric,
        allow_use_cicp_transfer: false,
        prefer_fixed_point: false,
        allow_extended_range_rgb_xyz: true,
        interpolation_method: InterpolationMethod::Tetrahedral,
        ..Default::default()
    }
}

/// XYZ to a profile's device values, or back: its own table where it has one, read stage by
/// stage, else its matrix and curves through the CMM.
#[derive(Clone)]
pub enum Leg {
    Matrix(Arc<TransformF32Executor>),
    Table(Arc<Table>),
}

impl Leg {
    /// Device values to D50 XYZ.
    pub fn to_xyz(profile: &ColorProfile) -> Option<Leg> {
        if let Some(table) = &profile.lut_a_to_b_colorimetric {
            return Some(Leg::Table(Arc::new(Table::new(table, profile.pcs, true)?)));
        }
        if !profile.is_matrix_shaper() {
            return None;
        }
        let t = profile.create_transform_f32(Layout::Rgba, &PCS_PROFILE, Layout::Rgba, relative()).ok()?;
        Some(Leg::Matrix(t))
    }

    /// D50 XYZ to device values.
    pub fn from_xyz(profile: &ColorProfile) -> Option<Leg> {
        if let Some(table) = &profile.lut_b_to_a_colorimetric {
            return Some(Leg::Table(Arc::new(Table::new(table, profile.pcs, false)?)));
        }
        if !profile.is_matrix_shaper() {
            return None;
        }
        let t = PCS_PROFILE.create_transform_f32(Layout::Rgba, profile, Layout::Rgba, relative()).ok()?;
        Some(Leg::Matrix(t))
    }

    fn run(&self, pixels: &[f32], table: impl Fn(&Table, [f32; 3]) -> [f32; 3]) -> Option<Vec<f32>> {
        match self {
            Leg::Matrix(t) => {
                let mut out = vec![0.0; pixels.len()];
                t.transform(pixels, &mut out).ok()?;
                Some(out)
            }
            Leg::Table(t) => Some(
                pixels.chunks_exact(4).flat_map(|p| {
                    let o = table(t, [p[0], p[1], p[2]]);
                    [o[0], o[1], o[2], p[3]]
                }).collect(),
            ),
        }
    }

    /// RGBA XYZ to RGBA device values, unclipped.
    pub fn device(&self, xyz: &[f32]) -> Option<Vec<f32>> {
        self.run(xyz, Table::device)
    }

    /// RGBA device values to RGBA XYZ.
    pub fn xyz(&self, device: &[f32]) -> Option<Vec<f32>> {
        self.run(device, Table::xyz)
    }
}

/// XYZ to the profile, with the black point compensation laid before it.
pub struct Legs {
    to_profile: Leg,
    pub black: f32,
    pub white: [f32; 3],
}

/// The profile read back to working light with what a print adds: the paper's white and the
/// chroma a CMM wrongly reads on it.
#[derive(Clone)]
pub struct Back {
    to_xyz: Leg,
    pub paper: [f32; 3],
    /// The a* and b* read on the device white, which is neutral by definition.
    pub offset: [f32; 2],
}

pub fn working_to_xyz(pixels: &mut [f32]) {
    for p in pixels.chunks_exact_mut(4) {
        let o = pcs::mul_f32(&WORKING_TO_PCS, [p[0], p[1], p[2]]);
        p[..3].copy_from_slice(&o);
    }
}

pub fn xyz_to_working(pixels: &mut [f32]) {
    for p in pixels.chunks_exact_mut(4) {
        let o = pcs::mul_f32(&PCS_TO_WORKING, [p[0], p[1], p[2]]);
        p[..3].copy_from_slice(&o);
    }
}

pub fn legs(data: &[u8]) -> Option<Legs> {
    let target = profile::parse(data)?;
    let to_profile = Leg::from_xyz(&target)?;
    let w = pcs::mul_f32(&WORKING_TO_PCS, [1.0; 3]);
    let black = Leg::to_xyz(&target)?.xyz(&[0.0, 0.0, 0.0, 1.0])?;
    if w[1] <= 0.0 {
        return None;
    }
    Some(Legs { to_profile, black: (black[1] / w[1]).max(0.0), white: w })
}

pub fn legs_back(data: &[u8]) -> Option<Back> {
    let target = profile::parse(data)?;
    let to_xyz = Leg::to_xyz(&target)?;
    let white = to_xyz.xyz(&[1.0, 1.0, 1.0, 1.0])?;
    let read = pcs::lab([white[0], white[1], white[2]]);
    // A screen's `wtpt` is its own white, D65 on sRGB: only a printer's names a paper.
    let paper = if OutputProfile::is_printer(data) { media_white(&target).unwrap_or(PCS_WHITE) } else { PCS_WHITE };
    Some(Back { to_xyz, paper, offset: [read[1], read[2]] })
}

pub fn media_white(profile: &ColorProfile) -> Option<[f32; 3]> {
    let w = profile.media_white_point?;
    (w.y > 0.0).then_some([w.x as f32, w.y as f32, w.z as f32])
}

impl Back {
    /// The same reading with another chroma offset, which a check uses to show the offset matters.
    pub fn with_offset(&self, offset: [f32; 2]) -> Back {
        Back { offset, ..self.clone() }
    }
}

/// Working-space RGBA to device values, clipped to [0, 1].
pub fn run(legs: &Legs, pixels: &[f32]) -> Option<Vec<f32>> {
    let mut xyz = pixels.to_vec();
    working_to_xyz(&mut xyz);
    compensate(&mut xyz, legs.black, legs.white);
    let mut device = legs.to_profile.device(&xyz)?;
    for v in &mut device {
        *v = v.clamp(0.0, 1.0);
    }
    Some(device)
}

/// Device values back to working light with the paper simulated; `undoing` shows colour alone.
pub fn run_back(back: &Back, device: &[f32], undoing: Option<&Legs>) -> Option<Vec<f32>> {
    let mut xyz = back.to_xyz.xyz(device)?;
    neutralise(&mut xyz, back.offset);
    match undoing {
        Some(legs) => uncompensate(&mut xyz, legs.black, legs.white),
        None => {
            let scale: [f32; 3] = std::array::from_fn(|c| back.paper[c] / PCS_WHITE[c]);
            for p in xyz.chunks_exact_mut(4) {
                for c in 0..3 {
                    p[c] *= scale[c];
                }
            }
        }
    }
    xyz_to_working(&mut xyz);
    Some(xyz)
}

/// Takes the CMM's chroma offset off every reading: a CMM may read a v4 Lab table's neutral off
/// grey, and the device white is neutral by definition.
pub fn neutralise(xyz: &mut [f32], offset: [f32; 2]) {
    // Written as the test that passes, so an offset read as NaN leaves the values alone.
    let moved = |v: f32| v.abs() > 1e-4;
    if !(moved(offset[0]) || moved(offset[1])) {
        return;
    }
    for p in xyz.chunks_exact_mut(4) {
        let read = pcs::lab([p[0], p[1], p[2]]);
        let fixed = pcs::xyz_from_lab([read[0], read[1] - offset[0], read[2] - offset[1]]);
        p[..3].copy_from_slice(&fixed);
    }
}

/// Black point compensation on a neutral black: `XYZ · (1 − b) + b · white`.
pub fn compensate(xyz: &mut [f32], black: f32, white: [f32; 3]) {
    let gain = 1.0 - black;
    for p in xyz.chunks_exact_mut(4) {
        for c in 0..3 {
            p[c] = p[c] * gain + black * white[c];
        }
    }
}

/// The exact inverse of `compensate`.
pub fn uncompensate(xyz: &mut [f32], black: f32, white: [f32; 3]) {
    if black >= 1.0 || black.is_nan() {
        return;
    }
    let gain = 1.0 / (1.0 - black);
    for p in xyz.chunks_exact_mut(4) {
        for c in 0..3 {
            p[c] = p[c] * gain - black * white[c] / (1.0 - black);
        }
    }
}

/// A converted frame, RGBX with X at full scale, rows top first.
#[derive(Debug, Clone, PartialEq)]
pub enum Quantised {
    Eight(Vec<u8>),
    Sixteen(Vec<u16>),
}

/// Rows converted at a time: a full float frame costs 750 MB.
pub const BAND: usize = 256;

impl OutputProfile {
    /// The frame into this profile, quantised to `bits` (8 or 16). `render(top, rows, out)` fills
    /// one band of working-space RGBA rows, top first; the clip and the rounding are done here.
    pub fn converted(
        &self,
        width: usize,
        height: usize,
        bits: u32,
        mut render: impl FnMut(usize, usize, &mut [f32]),
    ) -> Option<Quantised> {
        if !(bits == 8 || bits == 16) || width == 0 || height == 0 {
            return None;
        }
        let loaded = self.loaded()?;
        let legs = legs(&loaded.data)?;
        let samples = width * 4;
        let scale = if bits == 16 { 65535.0 } else { 255.0 };
        let mut out16 = Vec::new();
        let mut out8 = Vec::new();
        let mut source = vec![0.0f32; samples * BAND];
        for top in (0..height).step_by(BAND) {
            let rows = BAND.min(height - top);
            let band = &mut source[..samples * rows];
            render(top, rows, band);
            for p in band.chunks_exact_mut(4) {
                p[3] = 1.0;
            }
            let device = run(&legs, band)?;
            if bits == 16 {
                out16.extend(device.iter().map(|v| (v * scale).round() as u16));
            } else {
                out8.extend(device.iter().map(|v| (v * scale).round() as u8));
            }
        }
        Some(if bits == 16 { Quantised::Sixteen(out16) } else { Quantised::Eight(out8) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: [f32; 3] = PCS_WHITE;

    #[test]
    fn compensation_lifts_zero_onto_the_paper_black_and_keeps_white() {
        let black = 0.0131;
        let mut axis = vec![0.0, 0.0, 0.0, 1.0, 0.01 * WHITE[0], 0.01, 0.01 * WHITE[2], 1.0, WHITE[0], WHITE[1], WHITE[2], 1.0];
        compensate(&mut axis, black, WHITE);
        assert!((axis[1] - black).abs() < 1e-6 && (axis[9] - 1.0).abs() < 1e-6 && (axis[8] - WHITE[0]).abs() < 1e-6);
        assert!((axis[4] / axis[5] - WHITE[0]).abs() < 1e-5 && axis[5] > axis[1] + 0.009);
        // Twin: uncompensated, zero stays under the black the paper can print.
        let mut bare = vec![0.0, 0.0, 0.0, 1.0];
        compensate(&mut bare, 0.0, WHITE);
        assert!(bare[1] == 0.0 && bare[1] < axis[1]);
    }

    #[test]
    fn a_nan_reading_leaves_the_values_alone() {
        let original = vec![0.3, 0.2, 0.1, 1.0];
        let mut xyz = original.clone();
        neutralise(&mut xyz, [f32::NAN, 0.0]);
        uncompensate(&mut xyz, f32::NAN, WHITE);
        assert_eq!(xyz, original);
        // Twin: a finite offset does move them.
        neutralise(&mut xyz, [0.5, 0.0]);
        assert!(xyz.iter().zip(&original).any(|(a, b)| (a - b).abs() > 1e-4));
    }

    #[test]
    fn uncompensate_is_the_exact_inverse() {
        let original = vec![0.3, 0.2, 0.1, 1.0, 0.0, 0.0, 0.0, 1.0];
        let mut xyz = original.clone();
        compensate(&mut xyz, 0.03, WHITE);
        uncompensate(&mut xyz, 0.03, WHITE);
        assert!(xyz.iter().zip(&original).all(|(a, b)| (a - b).abs() < 1e-6));
        // Twin: undone with another black, the values do not come back.
        let mut wrong = original.clone();
        compensate(&mut wrong, 0.03, WHITE);
        uncompensate(&mut wrong, 0.02, WHITE);
        assert!(wrong.iter().zip(&original).any(|(a, b)| (a - b).abs() > 1e-3));
    }
}
