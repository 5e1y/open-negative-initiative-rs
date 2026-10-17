//! The keycap chrome, rasterised once per colour and height from the Frustum light, then nine-sliced
//! by Slint to each key's width: the face's dish and the walls vary only across the cap's height.

use oni_ui_poc::keycap::{self, Style};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

/// `Keycap.Variant.mini.factor`, the variant of the panel's small keys.
pub const MINI: f64 = 0.38;
/// `KeycapStyle.paddingH` before the variant scales it.
pub const PADDING_H: f64 = 32.0;
/// The raster's width against its height: room for both rounded ends and a middle to stretch.
const WIDTH_PER_HEIGHT: f64 = 2.0;

/// One chrome: the albedo, the variant's factor and the key's height in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chrome {
    pub albedo: [f64; 3],
    pub factor: f64,
    pub height: f64,
}

fn rgb(hex: u32) -> [f64; 3] {
    [(hex >> 16) & 0xff, (hex >> 8) & 0xff, hex & 0xff].map(|c| c as f64 / 255.0)
}

impl Chrome {
    pub fn small(hex: u32, height: f64) -> Chrome {
        Chrome { albedo: rgb(hex), factor: keycap::SMALL, height }
    }

    pub fn style(&self) -> Style {
        let mut s = Style::hardened().scaled(self.factor);
        s.albedo = self.albedo;
        s
    }

    /// `KeycapStyle.labelCentring`: the face sits above the cap's middle by half the extra lower wall.
    pub fn centring(&self) -> f64 {
        let s = self.style();
        -(s.wall_bottom - s.wall_top) / 2.0
    }

    /// The slice kept whole at each end, in points: the corner and the wall with its seam's blur.
    pub fn end(&self) -> f64 {
        let s = self.style();
        s.corner_radius.max(s.face_radius + s.wall_left.max(s.wall_right)) + 2.0 * s.seam_blur
    }

    /// Premultiplied RGBA8 at `scale` device pixels per point, with its nine-slice edges set.
    pub fn image(&self, scale: f64) -> Image {
        let width = (self.height * WIDTH_PER_HEIGHT).max(2.0 * self.end() + 2.0);
        let (w, h, bytes) = keycap::cap_pixels(&self.style(), width, self.height, scale, false);
        let mut image = Image::from_rgba8_premultiplied(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&bytes, w as u32, h as u32));
        let end = (self.end() * scale).ceil() as u16;
        let half = (h as u16 / 2).saturating_sub(1);
        image.set_nine_slice_edges(half, end, half, end);
        image
    }
}

/// The knob of a track in `albedo`, at `scale`.
pub fn knob(albedo: [f64; 3], scale: f64) -> Image {
    let mut style = Style::knob();
    style.albedo = albedo;
    let (w, h, bytes) = keycap::knob_pixels(&style, scale);
    Image::from_rgba8_premultiplied(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&bytes, w as u32, h as u32))
}

/// The colours the bar and the panel give their keys, `DS.Palette` and `DS.Tint`.
pub mod hex {
    pub const KEYCAP: u32 = 0x313131;
    pub const PALE: u32 = 0xffffff;
    /// `Color.accentColor` in the dark appearance.
    pub const ACCENT: u32 = 0x0a84ff;
    pub const GRINDING: u32 = 0xe26d1f;
    pub const UNDOABLE: u32 = 0xe0a814;
    pub const DESTRUCTIVE: u32 = 0xd4333b;
    pub const CORRECTION: u32 = 0x8a63d2;
    pub const CONFIRM: u32 = 0x2ea062;
    pub const IMPORTING: u32 = 0xff375f;
    pub const UPDATE: u32 = 0xff9f0a;
}

/// Every raster the Slint `Caps` global holds, at the window's scale.
pub fn install(caps: &crate::Caps, scale: f64) {
    use hex::*;
    let bar = 40.0;
    caps.set_grey(Chrome::small(KEYCAP, bar).image(scale));
    caps.set_pale(Chrome::small(PALE, bar).image(scale));
    caps.set_accent(Chrome::small(ACCENT, bar).image(scale));
    caps.set_grinding(Chrome::small(GRINDING, bar).image(scale));
    caps.set_undoable(Chrome::small(UNDOABLE, bar).image(scale));
    caps.set_destructive(Chrome::small(DESTRUCTIVE, bar).image(scale));
    caps.set_correction(Chrome::small(CORRECTION, bar).image(scale));
    caps.set_confirm(Chrome::small(CONFIRM, bar).image(scale));
    caps.set_importing(Chrome::small(IMPORTING, bar).image(scale));
    caps.set_update(Chrome::small(UPDATE, bar).image(scale));
    let mini = Chrome { factor: MINI, ..Chrome::small(KEYCAP, 26.0) };
    caps.set_mini_grey(mini.image(scale));
    caps.set_plate_grey(Chrome::small(KEYCAP, 52.0).image(scale));
    caps.set_knob_grey(knob(rgb(KEYCAP), scale));
    caps.set_knob_white(knob(rgb(0xffffff), scale));
    caps.set_knob_black(knob([0.0; 3], scale));
    let small = Chrome::small(KEYCAP, bar);
    caps.set_small_centring(small.centring() as f32);
    caps.set_mini_centring(mini.centring() as f32);
    caps.set_small_padding((PADDING_H * small.factor) as f32);
    caps.set_mini_padding((PADDING_H * mini.factor) as f32);
    caps.set_small_corner(small.style().corner_radius as f32);
    caps.set_mini_corner(mini.style().corner_radius as f32);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_variants_centre_their_labels_as_the_swift_style_does() {
        assert!((Chrome::small(hex::KEYCAP, 40.0).centring() + 1.05).abs() < 1e-9);
        let mini = Chrome { factor: MINI, ..Chrome::small(hex::KEYCAP, 26.0) };
        assert!((mini.centring() + 0.76).abs() < 1e-9);
        // Twin: equal walls would centre the label on the cap.
        let mut flat = Chrome::small(hex::KEYCAP, 40.0).style();
        flat.wall_bottom = flat.wall_top;
        assert_eq!(flat.wall_bottom - flat.wall_top, 0.0);
    }

    #[test]
    fn the_middle_column_is_uniform_so_slicing_it_loses_nothing() {
        let chrome = Chrome::small(hex::ACCENT, 40.0);
        let scale = 2.0;
        let width = (chrome.height * WIDTH_PER_HEIGHT).max(2.0 * chrome.end() + 2.0);
        let (w, h, px) = keycap::cap_pixels(&chrome.style(), width, chrome.height, scale, false);
        let end = (chrome.end() * scale).ceil() as usize;
        assert!(w > 2 * end + 1, "a middle remains between the two ends");
        for y in 0..h {
            let a = &px[(y * w + end) * 4..(y * w + end) * 4 + 4];
            let b = &px[(y * w + w - end - 1) * 4..(y * w + w - end - 1) * 4 + 4];
            assert!(a.iter().zip(b).all(|(p, q)| p.abs_diff(*q) <= 1), "row {y}: {a:?} against {b:?}");
        }
        // Twin: inside the rounded end the columns differ, which is why the ends are kept whole.
        assert_ne!(px[(w + 1) * 4 + 3], px[(w + end) * 4 + 3]);
    }
}
