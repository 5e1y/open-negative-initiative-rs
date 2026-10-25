//! The negative's screen: the measure's tile reduced and encoded on the measure's thread, its lights
//! blooming into the glass as a tube's phosphor spills its brightest.

use crate::display::{Params, Screen};
use crate::render::Linear;
use crate::roll_previews::Bitmap;
use oni_keycap::blur::{self, Plane};

/// The negative's long side on its screen, in pixels: twice the widest the bank lays it.
const SIDE: usize = 480;
/// What is lit past `BLOOM_FROM` spreads `BLOOM_SIGMA` pixels at `BLOOM` strength.
const BLOOM_FROM: f32 = 0.6;
const BLOOM_SIGMA: f32 = 3.0;
const BLOOM: f32 = 0.6;

/// The tile as the negative's screen shows it: reduced, on sRGB, its lights blooming into the glass.
pub(in crate::app) fn of(tile: &oni_measure::sheet::Picture) -> Option<Bitmap> {
    let (w, h) = (tile.width, tile.height);
    if w == 0 || h == 0 {
        return None;
    }
    let step = w.max(h).div_ceil(SIDE).max(1);
    let (nw, nh) = (w.div_ceil(step), h.div_ceil(step));
    let mut pixels = vec![[0.0f32; 4]; nw * nh];
    for (i, p) in pixels.iter_mut().enumerate() {
        let (x0, y0) = ((i % nw) * step, (i / nw) * step);
        let mut n = 0.0;
        for y in y0..(y0 + step).min(h) {
            for x in x0..(x0 + step).min(w) {
                let s = tile.pixels[y * w + x];
                (0..4).for_each(|c| p[c] += s[c]);
                n += 1.0;
            }
        }
        p.iter_mut().for_each(|v| *v /= n);
    }
    let mut rgba = crate::render::encoded(&Linear { width: nw, height: nh, pixels }, &Params::new(Screen::Srgb, nw as u32, nh as u32, 0), 1);
    bloomed(&mut rgba, nw, nh);
    Some(Bitmap { rgba, width: nw, height: nh })
}

/// What is lit past `BLOOM_FROM`, blurred and added back, as a tube's phosphor spills its brightest.
fn bloomed(rgba: &mut [u8], w: usize, h: usize) {
    let mut lit = Plane::new(w, h, 3);
    for (i, p) in rgba.chunks_exact(4).enumerate() {
        (0..3).for_each(|c| lit.data[i * 3 + c] = ((f32::from(p[c]) / 255.0 - BLOOM_FROM) / (1.0 - BLOOM_FROM)).max(0.0));
    }
    blur::gaussian(&mut lit, BLOOM_SIGMA);
    for (i, p) in rgba.chunks_exact_mut(4).enumerate() {
        (0..3).for_each(|c| p[c] = (f32::from(p[c]) + BLOOM * lit.data[i * 3 + c] * 255.0).round().min(255.0) as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_negative_is_reduced_onto_its_screen_and_only_its_lights_bloom() {
        let mut tile = oni_measure::sheet::Picture { width: 1200, height: 800, pixels: vec![[0.0, 0.0, 0.0, 1.0]; 960_000] };
        let b = of(&tile).unwrap();
        assert_eq!((b.width, b.height), (400, 267));
        assert!(b.rgba.chunks_exact(4).all(|p| p[0] < 4), "black stays black");
        // A lit spot spills past itself; twin: a grey under the threshold keeps its own value.
        let mut spot = vec![0u8; 21 * 21 * 4];
        spot[(10 * 21 + 10) * 4] = 255;
        bloomed(&mut spot, 21, 21);
        assert!(spot[(10 * 21 + 12) * 4] > 0, "the light spreads");
        let mut grey = vec![120u8; 9 * 9 * 4];
        bloomed(&mut grey, 9, 9);
        assert!(grey.iter().all(|v| *v == 120));
        tile.width = 0;
        assert!(of(&tile).is_none());
    }
}
