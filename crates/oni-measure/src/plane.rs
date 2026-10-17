//! A rendered image read back to the CPU, and the box of it Core Image would hand a bitmap.

use oni_settings::cg::Rect;

/// Rows top first; seen in its own Core Image frame, origin at the lower-left corner, y up.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f32; 4]>,
}

impl Plane {
    pub fn new(width: usize, height: usize, pixels: Vec<[f32; 4]>) -> Plane {
        assert_eq!(pixels.len(), width * height, "{width}x{height} needs {} pixels", width * height);
        Plane { width, height, pixels }
    }

    /// One colour over the whole plane.
    pub fn flat(width: usize, height: usize, rgba: [f32; 4]) -> Plane {
        Plane::new(width, height, vec![rgba; width * height])
    }

    pub fn extent(&self) -> Rect {
        Rect::new(0.0, 0.0, self.width as f64, self.height as f64)
    }

    /// The pixel whose lower-left corner is (`x`, `y`) in Core Image's frame; clear outside the extent.
    pub fn at_ci(&self, x: i64, y: i64) -> [f32; 4] {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return [0.0; 4];
        }
        self.pixels[(self.height - 1 - y as usize) * self.width + x as usize]
    }

    /// `CIContext.render(toBitmap:bounds:)` over a box of whole pixels: its top row first.
    pub fn bitmap(&self, bounds: &Rect) -> (usize, usize, Vec<[f32; 4]>) {
        let (w, h) = (bounds.width.max(0.0) as usize, bounds.height.max(0.0) as usize);
        let (x0, top) = (bounds.x as i64, (bounds.y + bounds.height) as i64 - 1);
        let mut out = Vec::with_capacity(w * h);
        for row in 0..h as i64 {
            for column in 0..w as i64 {
                out.push(self.at_ci(x0 + column, top - row));
            }
        }
        (w, h, out)
    }

    /// The plane with `rgba` laid over a box given in Core Image's frame.
    pub fn painted(mut self, rect: &Rect, rgba: [f32; 4]) -> Plane {
        let (x0, y0) = (rect.x.max(0.0) as usize, rect.y.max(0.0) as usize);
        let x1 = ((rect.x + rect.width) as usize).min(self.width);
        let y1 = ((rect.y + rect.height) as usize).min(self.height);
        for y in y0..y1 {
            for x in x0..x1 {
                let row = self.height - 1 - y;
                self.pixels[row * self.width + x] = rgba;
            }
        }
        self
    }

    /// `Pipeline.counted`: every component held to 0…1, as the measuring path bounds a reading.
    pub fn counted(mut self) -> Plane {
        for p in &mut self.pixels {
            for v in p.iter_mut() {
                *v = v.clamp(0.0, 1.0);
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bitmap_runs_top_down_in_a_bottom_up_frame() {
        let p = Plane::new(2, 2, vec![[1.0; 4], [2.0; 4], [3.0; 4], [4.0; 4]]);
        // The lower-left pixel is the first of the bottom row.
        assert_eq!(p.at_ci(0, 0), [3.0; 4]);
        let (w, h, b) = p.bitmap(&Rect::new(0.0, 0.0, 2.0, 2.0));
        assert_eq!((w, h, b.as_slice()), (2, 2, p.pixels.as_slice()));
        // Twin: a box past the extent reads clear there.
        assert_eq!(p.bitmap(&Rect::new(1.0, 1.0, 2.0, 1.0)).2, vec![[2.0; 4], [0.0; 4]]);
        let painted = Plane::flat(3, 3, [0.0; 4]).painted(&Rect::new(0.0, 0.0, 1.0, 1.0), [1.0; 4]);
        assert_eq!(painted.pixels[6], [1.0; 4]);
        assert_ne!(painted.pixels[0], [1.0; 4]);
    }
}
