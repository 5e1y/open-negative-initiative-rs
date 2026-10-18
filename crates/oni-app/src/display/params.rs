//! The display kernel's arguments: the screen's matrix, the dither's seed, the zebra's stripes.

use super::*;

/// The kernel's uniform block: the matrix by rows, width, height, seed and the dither's amplitude,
/// then `ScreenGamut`'s zebra: the screen's extended range, its tint, where a texel lands on the canvas.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Params {
    pub rows: [[f32; 4]; 3],
    pub width: u32,
    pub height: u32,
    pub seed: u32,
    /// In quantisation steps; 0 turns the dither off.
    pub dither: f32,
    pub low: [f32; 4],
    pub high: [f32; 4],
    /// Working light, as a pixel of the picture is.
    pub tint: [f32; 4],
    /// The texture's corner on the canvas, the canvas pixels per texel, the canvas's height.
    pub origin: [f32; 2],
    pub stretch: f32,
    pub canvas_height: f32,
    pub phase: f32,
    pub period: f32,
    pub duty: f32,
    /// 1 stripes what the screen cannot show, 0 leaves the picture alone.
    pub zebra: f32,
    /// The canvas's ground in working light, under a texel as clear as its alpha says.
    pub ground: f32,
    pub pad: [f32; 3],
}

/// `DS.Palette.canvas`, 27 of 255 in sRGB, as light: what `MetalImageView` composites the picture over.
pub const GROUND: f32 = 0.010_960_094;

/// Where the zebra is laid: the texture's place on the canvas, in drawable pixels, and the clock.
#[derive(Clone, Copy, Debug)]
pub struct Zebra {
    pub origin: (f32, f32),
    pub stretch: f32,
    pub canvas_height: f32,
    pub phase: f32,
}

impl Params {
    pub fn new(screen: Screen, width: u32, height: u32, seed: u32) -> Params {
        let m = screen.matrix();
        Params {
            rows: m.map(|r| [r[0], r[1], r[2], 0.0]),
            width,
            height,
            seed,
            dither: 1.0,
            low: [0.0; 4],
            high: [1.0; 4],
            tint: [0.0; 4],
            origin: [0.0; 2],
            stretch: 1.0,
            canvas_height: height as f32,
            phase: 0.0,
            period: oni_colour::gamut::PERIOD,
            duty: oni_colour::gamut::DUTY,
            zebra: 0.0,
            ground: GROUND,
            pad: [0.0; 3],
        }
    }

    /// With the zebra on: the range is the screen's, mirrored under zero and continued past one by
    /// `TOLERANCE`, as the system extends a display's space; sRGB and Display P3 share the curve.
    pub fn striped(self, z: Zebra) -> Params {
        let tolerance = oni_colour::gamut::TOLERANCE;
        let decode = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
        let (low, high) = (-decode(tolerance), decode(1.0 + tolerance));
        Params {
            low: [low, low, low, 0.0],
            high: [high, high, high, 0.0],
            tint: oni_colour::gamut::tint(),
            origin: [z.origin.0, z.origin.1],
            stretch: z.stretch,
            canvas_height: z.canvas_height,
            phase: z.phase,
            zebra: 1.0,
            ..self
        }
    }
}
