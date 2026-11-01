//! Ink on a clear plate, as the sheet's numbers and heading are set: text in the interface's face,
//! bars and the logo, in sRGB premultiplied by coverage, laid over the sheet's black or shown as is.

use oni_export::Image as Picture;
use swash::scale::{Render, ScaleContext, Source as Outline};
use swash::zeno::Format as Mask;

/// The interface's face, which the numbers are set in on screen.
const FACE: &[u8] = include_bytes!("../../../../oni-ui/fonts/Inter.ttf");

/// The numbers' ink as the design system states it, in sRGB, and their weight.
#[derive(Clone, Copy, Debug)]
pub(in crate::app) struct Lettering {
    pub ink: [f32; 3],
    pub weight: f32,
}

/// Where a line of text stands on its pen's position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Anchor {
    Start,
    Centre,
    End,
}

/// A clear plate: each pixel's ink in sRGB times its coverage, and the coverage.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Plate {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f32; 4]>,
}

impl Plate {
    pub fn clear(width: usize, height: usize) -> Plate {
        Plate { width, height, pixels: vec![[0.0; 4]; width * height] }
    }

    /// `ink` at `coverage` on the pixel at `x`, `y`; where two marks touch the fuller one shows.
    pub fn cover(&mut self, x: i64, y: i64, coverage: f32, ink: [f32; 3]) {
        if coverage <= 0.0 || x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return;
        }
        let p = &mut self.pixels[y as usize * self.width + x as usize];
        if coverage > p[3] {
            *p = [ink[0] * coverage, ink[1] * coverage, ink[2] * coverage, coverage];
        }
    }

    /// A solid rectangle of `ink`, its edges on whole pixels.
    pub fn rect(&mut self, (x, y, w, h): (i64, i64, i64, i64), ink: [f32; 3]) {
        for row in y..y + h {
            for col in x..x + w {
                self.cover(col, row, 1.0, ink);
            }
        }
    }

    /// `text` at `size` pixels and `weight`, its baseline at `at.1`, standing on `at.0` as `anchor` says.
    pub fn text(&mut self, at: (f32, f32), text: &str, size: f32, lettering: Lettering, anchor: Anchor, context: &mut ScaleContext) {
        let Some(face) = Face::of(lettering.weight) else { return };
        let width = face.width(text, size);
        let start = match anchor {
            Anchor::Start => at.0,
            Anchor::Centre => at.0 - width / 2.0,
            Anchor::End => at.0 - width,
        };
        let font = face.font();
        let mut scaler = context.builder(font).size(size).normalized_coords(&face.coords).build();
        let advances = font.glyph_metrics(&face.coords).scale(size);
        let mut pen = start;
        for c in text.chars() {
            let glyph = font.charmap().map(c);
            if let Some(mask) = Render::new(&[Outline::Outline]).format(Mask::Alpha).render(&mut scaler, glyph) {
                let p = mask.placement;
                let (left, top) = (i64::from(pen.round() as i32 + p.left), i64::from(at.1.round() as i32 - p.top));
                for (i, &a) in mask.data.iter().enumerate() {
                    let (x, y) = (left + (i % p.width as usize) as i64, top + (i / p.width as usize) as i64);
                    self.cover(x, y, f32::from(a) / 255.0, lettering.ink);
                }
            }
            pen += advances.advance_width(glyph);
        }
    }

    /// The plate as straight RGBA8, as the canvas shows it.
    pub fn rgba8(&self) -> Vec<u8> {
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        self.pixels.iter().flat_map(|p| {
            let a = p[3];
            let straight = |v: f32| if a > 0.0 { v / a } else { 0.0 };
            [byte(straight(p[0])), byte(straight(p[1])), byte(straight(p[2])), byte(a)]
        }).collect()
    }

    /// The plate laid over `sheet`'s black at `at`, its ink linearised to the working space as the
    /// screen shows it blended there.
    pub fn lay_on(&self, sheet: &mut Picture, at: (i64, i64)) {
        let srgb = oni_colour::source::Source::of(None, None, false);
        let mut inked: Vec<[f32; 4]> = self.pixels.iter().map(|p| [p[0], p[1], p[2], 1.0]).collect();
        srgb.linearise(&mut inked, false);
        for (i, (p, ink)) in self.pixels.iter().zip(&inked).enumerate() {
            let (x, y) = (at.0 + (i % self.width) as i64, at.1 + (i / self.width) as i64);
            if p[3] <= 0.0 || x < 0 || y < 0 || x >= sheet.width as i64 || y >= sheet.height as i64 {
                continue;
            }
            let d = &mut sheet.pixels[(y as usize * sheet.width + x as usize) * 4..][..3];
            for (d, v) in d.iter_mut().zip(ink) {
                *d = d.max(*v);
            }
        }
    }
}

/// The interface's face at a weight, and its measures.
pub(super) struct Face {
    coords: Vec<i16>,
}

impl Face {
    pub fn of(weight: f32) -> Option<Face> {
        let font = swash::FontRef::from_index(FACE, 0)?;
        let axes = [(b"wght", weight)];
        Some(Face { coords: font.variations().normalized_coords(&axes).collect() })
    }

    fn font(&self) -> swash::FontRef<'static> {
        swash::FontRef::from_index(FACE, 0).expect("the bundled face reads")
    }

    /// The width `text` takes at `size` pixels.
    pub fn width(&self, text: &str, size: f32) -> f32 {
        let font = self.font();
        let advances = font.glyph_metrics(&self.coords).scale(size);
        text.chars().map(|c| advances.advance_width(font.charmap().map(c))).sum()
    }

    /// The baseline that centres a line of `size` pixels in a band from `top`, `height` tall, as a
    /// label centres its text.
    pub fn centred(&self, top: f32, height: f32, size: f32) -> f32 {
        let m = self.font().metrics(&self.coords).scale(size);
        top + (height - (m.ascent + m.descent)) / 2.0 + m.ascent
    }
}

/// The design system's orange in bold, as the checks set it.
#[cfg(test)]
pub(super) const ORANGE: Lettering = Lettering { ink: [1.0, 159.0 / 255.0, 10.0 / 255.0], weight: 700.0 };

#[cfg(test)]
mod tests {
    use super::*;

    /// The columns and rows of `p` holding any ink.
    fn inked(p: &Plate) -> (Vec<usize>, Vec<usize>) {
        let on = |x: usize, y: usize| p.pixels[y * p.width + x][3] > 0.0;
        ((0..p.width).filter(|&x| (0..p.height).any(|y| on(x, y))).collect(), (0..p.height).filter(|&y| (0..p.width).any(|x| on(x, y))).collect())
    }

    #[test]
    fn text_stands_on_its_anchor_and_centres_in_its_band() {
        let face = Face::of(700.0).unwrap();
        let width = face.width("12", 56.0);
        let mut plate = Plate::clear(400, 60);
        let baseline = face.centred(0.0, 60.0, 56.0);
        plate.text((390.0, baseline), "12", 56.0, ORANGE, Anchor::End, &mut ScaleContext::new());
        let (columns, rows) = inked(&plate);
        assert!((*columns.last().unwrap() as f32 - 390.0).abs() < 6.0 && (columns[0] as f32 - (390.0 - width)).abs() < 8.0, "ends on its anchor: {columns:?}");
        let (top, bottom) = (rows[0], *rows.last().unwrap());
        assert!(top > 2 && bottom < 58 && (top as i64 - (59 - bottom as i64)).abs() < 14, "centred in its band: rows {top}..{bottom}");
        // Twin: the start anchor sets it from its pen on.
        let mut start = Plate::clear(400, 60);
        start.text((10.0, baseline), "12", 56.0, ORANGE, Anchor::Start, &mut ScaleContext::new());
        assert!(inked(&start).0[0] < 20);
    }

    #[test]
    fn a_plate_lays_its_ink_linearised_over_black_and_shows_straight() {
        let mut plate = Plate::clear(2, 1);
        plate.cover(0, 0, 0.5, ORANGE.ink);
        let rgba = plate.rgba8();
        assert_eq!(&rgba[..4], &[255, 159, 10, 128], "straight ink, its coverage in alpha");
        let mut sheet = Picture { width: 2, height: 1, pixels: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0] };
        plate.lay_on(&mut sheet, (0, 0));
        assert!(sheet.pixels[0] > 0.1 && sheet.pixels[0] < 0.5, "half covered, darker than the ink: {:?}", &sheet.pixels[..4]);
        // Twin: a clear pixel leaves the black as it was.
        assert_eq!(&sheet.pixels[4..], &[0.0, 0.0, 0.0, 1.0]);
    }
}
