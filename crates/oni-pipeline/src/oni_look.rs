//! ONI's scanner look: a retouch of Pro's render in display sRGB, baked once into the cube the Scanner
//! face's ONI machine reads. Its fields at rest are the look.

use oni_render::ColorCube;
use std::sync::LazyLock;

/// One hue band, centred in OKLCh degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub centre: f32,
    pub hue: f32,
    pub saturation: f32,
    pub lightness: f32,
}

const fn band(centre: f32, hue: f32, saturation: f32, lightness: f32) -> Band {
    Band { centre, hue, saturation, lightness }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OniLook {
    pub exposure: f32,
    pub contrast: f32,
    pub channel_contrast: [f32; 3],
    pub offset: [f32; 3],
    /// Each channel pushed from the other two, in stops: red from green and blue, and so on.
    pub cross: [[f32; 2]; 3],
    pub curve_gamma: f32,
    pub curve_pivot: f32,
    pub hue_preservation: f32,
    pub output_black: f32,
    pub output_white: f32,
    pub saturation: f32,
    pub vibrance: f32,
    pub shadow_saturation: f32,
    pub highlight_saturation: f32,
    pub bands: [Band; 8],
    pub shadow_tint_hue: f32,
    pub shadow_tint: f32,
    pub highlight_tint_hue: f32,
    pub highlight_tint: f32,
}

impl Default for OniLook {
    fn default() -> Self {
        OniLook {
            exposure: 0.9976,
            contrast: 0.6730,
            channel_contrast: [0.9546, 1.0469, 0.9492],
            offset: [0.0468, 0.0, 0.0],
            cross: [[0.0; 2]; 3],
            curve_gamma: 1.68,
            curve_pivot: 0.46,
            hue_preservation: 0.5,
            output_black: 0.0298,
            output_white: 0.9701,
            saturation: 0.9505,
            vibrance: 0.0981,
            shadow_saturation: 1.0,
            highlight_saturation: 1.0,
            bands: [
                band(25.0, 4.70, 1.0, 0.0),
                band(60.0, 0.0, 1.0, 0.0),
                band(100.0, 4.61, 0.9533, 0.1047),
                band(145.0, 4.59, 0.9991, -0.0539),
                band(195.0, 4.55, 1.0, 0.0),
                band(255.0, 0.0, 1.0, 0.0),
                band(300.0, 0.0, 1.0, 0.0),
                band(345.0, 3.45, 1.0, 0.0),
            ],
            shadow_tint_hue: 199.4,
            shadow_tint: 0.0105,
            highlight_tint_hue: 147.6,
            highlight_tint: 0.0103,
        }
    }
}

pub const NODES: usize = 64;
/// The cube is indexed on this root of display code, which spends its nodes on the shadows.
pub const ROOT: f32 = 2.0;
const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

pub fn linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

pub fn code(l: f32) -> f32 {
    if l <= 0.003_130_8 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 }
}

fn oklab(c: [f32; 3]) -> [f32; 3] {
    let l = (0.412_221_46 * c[0] + 0.536_332_55 * c[1] + 0.051_445_995 * c[2]).cbrt();
    let m = (0.211_903_5 * c[0] + 0.680_699_5 * c[1] + 0.107_396_96 * c[2]).cbrt();
    let s = (0.088_302_46 * c[0] + 0.281_718_85 * c[1] + 0.629_978_7 * c[2]).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn linear_rgb(lab: [f32; 3]) -> [f32; 3] {
    let cube = |v: f32| v * v * v;
    let l = cube(lab[0] + 0.396_337_78 * lab[1] + 0.215_803_76 * lab[2]);
    let m = cube(lab[0] - 0.105_561_346 * lab[1] - 0.063_854_17 * lab[2]);
    let s = cube(lab[0] - 0.089_484_18 * lab[1] - 1.291_485_5 * lab[2]);
    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}

fn inside(v: [f32; 3]) -> bool {
    v.iter().all(|&c| (0.0..=1.0).contains(&c))
}

impl OniLook {
    /// The tone curve at display value `x`: an S of `curve_gamma` through half at the pivot.
    pub fn curve(&self, x: f32) -> f32 {
        let k = (self.curve_pivot / (1.0 - self.curve_pivot)).powf(self.curve_gamma);
        let (a, b) = (x.powf(self.curve_gamma), (1.0 - x).powf(self.curve_gamma));
        a / (a + k * b).max(1e-12)
    }

    /// Display sRGB code out for display sRGB code in.
    pub fn render(&self, input: [f32; 3]) -> [f32; 3] {
        let light = input.map(linear);
        let log = light.map(|l| (l.max(1e-6) / 0.18).log2());
        let others = [[1, 2], [0, 2], [0, 1]];
        let d: [f32; 3] = std::array::from_fn(|c| {
            log[c] + self.cross[c][0] * (log[c] - log[others[c][0]]) + self.cross[c][1] * (log[c] - log[others[c][1]])
        });
        let stops: [f32; 3] = std::array::from_fn(|c| d[c] * self.contrast * self.channel_contrast[c] + self.exposure + self.offset[c]);
        let scene: [f32; 3] = std::array::from_fn(|c| if light[c] <= 0.0 { 0.0 } else { 0.18 * stops[c].exp2() });
        let bend = |l: f32| linear(self.curve(code(l.clamp(0.0, 1.0))));
        let bent = scene.map(bend);
        let luma_in = (scene[0] * LUMA[0] + scene[1] * LUMA[1] + scene[2] * LUMA[2]).max(1e-6);
        let luma_out = bend(luma_in);
        let mut kept = scene.map(|v| v * (luma_out / luma_in));
        let top = kept.iter().copied().fold(f32::MIN, f32::max);
        // A channel past white walks toward the grey of the same light, never clipped alone.
        if top > 1.0 {
            let room = ((1.0 - luma_out) / (top - luma_out).max(1e-6)).clamp(0.0, 1.0);
            kept = kept.map(|v| luma_out + room * (v - luma_out));
        }
        let rgb: [f32; 3] = std::array::from_fn(|c| bent[c] + self.hue_preservation * (kept[c] - bent[c]));
        let coloured = self.coloured(rgb);
        coloured.map(|v| self.output_black + (self.output_white - self.output_black) * code(v.clamp(0.0, 1.0)))
    }

    /// Saturation, the bands and the split tint in OKLCh, then back inside sRGB by chroma alone.
    fn coloured(&self, rgb: [f32; 3]) -> [f32; 3] {
        let lab = oklab(rgb.map(|v| v.max(0.0)));
        let l = lab[0];
        let mut chroma = (lab[1] * lab[1] + lab[2] * lab[2]).sqrt();
        let mut hue = lab[2].atan2(lab[1]).to_degrees();
        if hue < 0.0 {
            hue += 360.0;
        }
        let coloured = (chroma / 0.04).min(1.0);
        let (mut shift, mut gain, mut lift, mut total) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for b in &self.bands {
            let mut distance = (hue - b.centre).abs();
            if distance > 180.0 {
                distance = 360.0 - distance;
            }
            let w = ((distance / 50.0).min(1.0) * std::f32::consts::FRAC_PI_2).cos().max(0.0);
            let weight = w * w;
            shift += weight * b.hue;
            gain += weight * (b.saturation - 1.0);
            lift += weight * b.lightness;
            total += weight;
        }
        let norm = total.max(1.0);
        hue += coloured * shift / norm;
        let zone = if l < 0.5 {
            self.shadow_saturation + (1.0 - self.shadow_saturation) * (l / 0.5)
        } else {
            1.0 + (self.highlight_saturation - 1.0) * ((l - 0.5) / 0.5)
        };
        chroma *= self.saturation * zone * (1.0 + coloured * gain / norm).max(0.0);
        chroma *= (1.0 + self.vibrance * (1.0 - (chroma / 0.25).min(1.0))).max(0.0);
        let lightness = l + coloured * lift / norm * 0.2;
        let radians = hue.to_radians();
        let (mut a, mut b) = (chroma * radians.cos(), chroma * radians.sin());
        let held = lightness.clamp(0.0, 1.0);
        let (shadow, highlight) = (1.0 - held, held);
        let (sh, hh) = (self.shadow_tint_hue.to_radians(), self.highlight_tint_hue.to_radians());
        a += self.shadow_tint * shadow * shadow * sh.cos() + self.highlight_tint * highlight * highlight * hh.cos();
        b += self.shadow_tint * shadow * shadow * sh.sin() + self.highlight_tint * highlight * highlight * hh.sin();
        let out = linear_rgb([lightness, a, b]);
        if inside(out) {
            return out;
        }
        let (mut keep, mut drop) = (0.0f32, 1.0f32);
        for _ in 0..10 {
            let mid = (keep + drop) / 2.0;
            if inside(linear_rgb([lightness, a * mid, b * mid])) { keep = mid } else { drop = mid }
        }
        linear_rgb([lightness, a * keep, b * keep])
    }

    /// The cube `CIColorCube` reads, red fastest: node (r, g, b) holds the render of the code its root stands for.
    pub fn cube(&self) -> ColorCube {
        let n = NODES;
        let step = 1.0 / (n - 1) as f32;
        let mut data = vec![[1.0f32; 4]; n * n * n];
        let workers = std::thread::available_parallelism().map_or(1, |w| w.get());
        let per = n.div_ceil(workers);
        std::thread::scope(|scope| {
            for (k, part) in data.chunks_mut(n * n * per).enumerate() {
                scope.spawn(move || {
                    for (i, node) in part.iter_mut().enumerate() {
                        let (r, g, b) = (i % n, (i / n) % n, k * per + i / (n * n));
                        let v = self.render([r, g, b].map(|j| (step * j as f32).powf(ROOT)));
                        *node = [v[0], v[1], v[2], 1.0];
                    }
                });
            }
        });
        ColorCube { cube_dimension: n, cube_data: data }
    }
}

/// The look's cube, baked once on first use.
pub static CUBE: LazyLock<ColorCube> = LazyLock::new(|| OniLook::default().cube());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_look_lifts_a_mid_grey_and_keeps_it_grey() {
        let look = OniLook::default();
        let grey = look.render([0.45; 3]);
        assert!(grey[0] > 0.45, "the look's exposure brightens: {grey:?}");
        assert!((grey[0] - grey[1]).abs() < 0.02 && (grey[1] - grey[2]).abs() < 0.03, "{grey:?}");
        let black = look.render([0.0; 3]);
        assert!((black[1] - look.output_black).abs() < 0.01, "{black:?}");
        // Twin: the code passed through untouched does not brighten.
        assert!(code(linear(0.45)) < grey[0]);
    }

    #[test]
    fn the_cube_is_red_fastest_on_rooted_code() {
        let cube = &*CUBE;
        assert_eq!(cube.cube_data.len(), NODES * NODES * NODES);
        let look = OniLook::default();
        let r = 40;
        let u = (r as f32 / (NODES - 1) as f32).powf(ROOT);
        let expect = look.render([u, 0.0, 0.0]);
        let node = cube.cube_data[r];
        assert!((node[0] - expect[0]).abs() < 1e-6 && (node[1] - expect[1]).abs() < 1e-6);
        // Twin: the same index read as blue is another colour.
        let blue = cube.cube_data[r * NODES * NODES];
        assert!((blue[0] - node[0]).abs() > 0.05);
    }
}
