//! The frame in pixels: the crop quantised on the full-resolution grid as stage 0 cuts it, and the
//! whole pixels it keeps, which every readout of the kept size says.

use super::Size;
use crate::frame::Geometry;

/// One pixel of the current source: the band over which the rotation blends edges with transparency.
const BLEED_MARGIN: f64 = 1.0;

/// A fraction on a grid of `divisions`, a whole division in or out; the grid's own float noise moves none.
fn quantised(fraction: f64, divisions: f64, up: bool) -> f64 {
    if divisions <= 0.0 {
        return fraction;
    }
    let v = fraction * divisions;
    let v = if (v - v.round()).abs() < 1e-9 { v.round() } else if up { v.ceil() } else { v.floor() };
    v / divisions
}

impl Geometry {
    pub fn cropped_pixel_size(&self, image: Size) -> Size {
        let s = self.oriented_size(image);
        (self.crop.width * s.0, self.crop.height * s.1)
    }

    /// The frame in the pixels of the upright box `[x, y, w, h]`, before rounding: the crop's
    /// bottom-left fractions quantised on the full-resolution grid, `scale` the box's width over it.
    pub fn frame_in(&self, upright: [f64; 4], scale: f64) -> [f64; 4] {
        let [x, y, w, h] = upright;
        let (gw, gh) = (w / scale, h / scale);
        let min_x = quantised(self.crop.x, gw, true);
        let min_y = quantised(self.crop.y, gh, true);
        let width = quantised(self.crop.width, gw, false);
        let height = quantised(self.crop.height, gh, false);
        let r = [x + min_x * w, y + (1.0 - min_y - height) * h, width * w, height * h];
        if self.angle == 0.0 {
            return r;
        }
        [r[0] + BLEED_MARGIN, r[1] + BLEED_MARGIN, r[2] - 2.0 * BLEED_MARGIN, r[3] - 2.0 * BLEED_MARGIN]
    }

    /// The crop in whole pixels as stage 0 cuts it from the full frame, every pixel the frame touches:
    /// the one size the crop's readout, its badge, the status bar, the export panel and the file share.
    pub fn kept_pixels(&self, image: Size) -> Size {
        let (w, h) = self.oriented_size(image);
        let [x, y, fw, fh] = self.frame_in([0.0, 0.0, w, h], 1.0);
        ((x + fw).ceil() - x.floor(), (y + fh).ceil() - y.floor())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cg::Rect;

    const RAW: Size = (6000.0, 4000.0);

    #[test]
    fn the_kept_size_is_whole_pixels_on_the_turned_photo_whichever_way_the_turn_is_written() {
        let g = Geometry { crop: Rect { x: 0.1, y: 0.0, width: 0.50009, height: 1.0 }, ..Geometry::default() };
        assert_eq!(g.kept_pixels(RAW), (3000.0, 4000.0), "the grid's whole pixels, as stage 0 cuts them");
        // A sliver of a row at each edge leaves both rows out; a tilted frame loses its bleed's two.
        let slivers = Geometry { crop: Rect { x: 0.0, y: 0.00001, width: 1.0, height: 0.99998 }, ..Geometry::default() };
        assert_eq!(slivers.frame_in([0.0, 0.0, 6000.0, 4000.0], 1.0), [0.0, 0.0, 6000.0, 3999.0]);
        assert_eq!(slivers.kept_pixels(RAW), (6000.0, 3999.0));
        let tilted = Geometry { angle: 1.0, ..g.clone() };
        assert_eq!(tilted.kept_pixels(RAW), (2998.0, 3998.0));
        // A box of no whole height keeps every row the frame touches.
        assert_eq!(Geometry::default().kept_pixels((6048.0, 4031.4)), (6048.0, 4032.0));
        // Twin: the readouts' former rounding said a pixel more, and a pixel less, than the file holds.
        assert_eq!(((0.50009f64 * RAW.0).round(), (0.99998f64 * RAW.1).round()), (3001.0, 4000.0));
        // A turn written backwards is still a quarter: the sides swap as the render's do.
        let back = Geometry { quarter_turns: -1, ..g.clone() };
        assert_eq!(back.kept_pixels(RAW), (2000.0, 6000.0));
        // Twin: the remainder's sign kept, −1 would read as no turn and the sides stay.
        assert_ne!(back.kept_pixels(RAW), Geometry { quarter_turns: 0, ..back.clone() }.kept_pixels(RAW));
    }
}
