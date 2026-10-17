//! The flat-field's masks: the shipped Valoi panel decoded with no colour conversion, its peak, the
//! arguments its kernel takes, and the stretch that lays a mask on the frame's extent.

use crate::stages::{self, Resample};
use oni_graph::{Image, Rect};
use oni_render::{Affine, ColorMatrix, FlatField as FlatFieldArgs};
use oni_settings::ConversionMode;
use oni_settings::frame::{FlatField, FlatFieldMask};

/// `FlatField.colourReach`: the exponent a full travel moves a channel's correction by.
pub const COLOUR_REACH: f32 = 0.5;

const VALOI: &[u8] = include_bytes!("../resources/flat-fields/valoi-easy35-v1.png");

/// A mask as the kernel reads it: its pixels, and the peaks it is normalised by.
#[derive(Clone, Debug)]
pub struct Mask {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[f32; 4]>,
    pub peaks: [f32; 3],
    /// Made of passes: normalised and divided channel by channel, any other on its green.
    pub per_channel: bool,
}

/// The Valoi Easy35 panel, read as Core Image reads it with no colour space: code over 255.
pub fn valoi() -> Mask {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(VALOI));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().expect("the shipped mask is a PNG");
    let mut buffer = vec![0; reader.output_buffer_size().expect("the mask's size")];
    let info = reader.next_frame(&mut buffer).expect("the shipped mask decodes");
    let channels = info.color_type.samples();
    assert_eq!(info.bit_depth, png::BitDepth::Eight, "the shipped mask is 8-bit");
    let pixels: Vec<[f32; 4]> = buffer[..info.buffer_size()]
        .chunks_exact(channels)
        .map(|c| {
            let v = |i: usize| f32::from(c[i.min(channels - 1)]) / 255.0;
            [v(0), v(1), v(2), if channels == 4 { v(3) } else { 1.0 }]
        })
        .collect();
    let peaks = pixels.iter().fold([0.0f32; 3], |m, p| [m[0].max(p[0]), m[1].max(p[1]), m[2].max(p[2])]);
    Mask { width: info.width, height: info.height, pixels, peaks, per_channel: false }
}

/// `FlatField.exponents`: the sense follows the mode, as the gains' does.
pub fn exponents(f: &FlatField, mode: ConversionMode) -> [f32; 3] {
    let sense = if mode.inverts_gain_sense() { 1.0 } else { -1.0 };
    let t = sense * COLOUR_REACH * f.temperature;
    let g = sense * COLOUR_REACH * f.tint;
    [1.0 - t / 2.0, 1.0 + g, 1.0 + t]
}

pub fn arguments(f: &FlatField, mode: ConversionMode, per_channel: bool) -> FlatFieldArgs {
    FlatFieldArgs { amount: f.amount, exponent: exponents(f, mode), per_channel: if per_channel { 1.0 } else { 0.0 } }
}

/// Whether the settings name a mask the pipeline holds itself.
pub fn shipped(f: &FlatField) -> bool {
    f.mask == FlatFieldMask::ValoiEasy35V1
}

/// `FlatField.stretched`: `mask` scaled onto `extent`, read bilinearly with its edge repeated,
/// divided by its peaks: green on all three unless the mask is per channel.
pub fn fitted(mask: Image, peaks: [f32; 3], per_channel: bool, extent: Rect) -> Image {
    let m = mask.extent();
    let (sx, sy) = (m.w as f32 / extent.w as f32, m.h as f32 / extent.h as f32);
    let k = Affine {
        m: [[sx, 0.0, m.x as f32 - extent.x as f32 * sx], [0.0, sy, m.y as f32 - extent.y as f32 * sy]],
        transparent: false,
    };
    let stretched = mask.then(Resample { k, extent });
    if peaks[1] <= 0.0 {
        return stretched;
    }
    let peaks = if per_channel { peaks } else { [peaks[1]; 3] };
    let inverse = peaks.map(|p| if p > 0.0 { 1.0 / p } else { 1.0 });
    stretched.then(stages::color_matrix(&ColorMatrix {
        r_vector: [inverse[0], 0.0, 0.0, 0.0],
        g_vector: [0.0, inverse[1], 0.0, 0.0],
        b_vector: [0.0, 0.0, inverse[2], 0.0],
        ..ColorMatrix::default()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_mask_decodes_whole_with_a_peak_under_one() {
        let m = valoi();
        assert_eq!((m.width, m.height, m.pixels.len()), (2000, 1333, 2000 * 1333));
        assert!(m.peaks[1] > 0.5 && m.peaks[1] <= 1.0, "{:?}", m.peaks);
        // The panel falls off: its corner sits under its peak.
        assert!(m.pixels[0][1] < m.peaks[1] * 0.99);
    }

    #[test]
    fn the_colour_sliders_turn_round_with_the_mode() {
        let f = FlatField { temperature: 1.0, ..FlatField::default() };
        assert_eq!(exponents(&f, ConversionMode::Negative), [0.75, 1.0, 1.5]);
        assert_eq!(exponents(&f, ConversionMode::Positive), [1.25, 1.0, 0.5]);
    }
}
