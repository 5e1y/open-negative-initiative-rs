//! Where a dust correction's patch lands and how its seam is feathered, as `CorrectionRenderer` places
//! them; the patches themselves are the caller's, generated once at full resolution.

use oni_graph::Rect;
use oni_settings::frame::Correction;

/// `CorrectionRenderer.tileSide`.
pub const TILE_SIDE: f64 = 800.0;

/// A rectangle in fractional pixels, y running down: x, y, width, height.
pub type Area = [f64; 4];

/// The feather softening a patch's seam, in `scale`'s pixels, floored at 2 px of full resolution.
pub fn feather(c: &Correction, scale: f64) -> f64 {
    (c.radius * 0.2).max(2.0) * scale
}

fn points(c: &Correction, extent: Area) -> impl Iterator<Item = (f64, f64)> + '_ {
    c.stroke.iter().map(move |p| (extent[0] + p.x * extent[2], extent[1] + p.y * extent[3]))
}

fn bounds(c: &Correction, extent: Area) -> Option<(f64, f64, f64, f64)> {
    let mut it = points(c, extent);
    let first = it.next()?;
    Some(it.fold((first.0, first.1, first.0, first.1), |b, p| (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))))
}

/// `contextRect`: an 800 px tile centred on the stroke, inside the extent, on whole pixels.
pub fn context_rect(c: &Correction, extent: Area) -> Area {
    let (w, h) = (TILE_SIDE.min(extent[2]), TILE_SIDE.min(extent[3]));
    let Some(b) = bounds(c, extent) else { return [extent[0], extent[1], w, h] };
    let (cx, cy) = ((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0);
    let x = (cx - w / 2.0).max(extent[0]).min((extent[0] + extent[2] - w).max(extent[0]));
    let y = (cy - h / 2.0).max(extent[1]).min((extent[1] + extent[3] - h).max(extent[1]));
    [x.round(), y.round(), w, h]
}

/// `strokeBoundingRect`: the stroke's footprint, its radius around every point.
pub fn stroke_bounding_rect(c: &Correction, extent: Area) -> Option<Area> {
    let b = bounds(c, extent)?;
    Some([b.0 - c.radius, b.1 - c.radius, b.2 - b.0 + 2.0 * c.radius, b.3 - b.1 + 2.0 * c.radius])
}

fn integral(a: Area) -> Area {
    let (x0, y0) = (a[0].floor(), a[1].floor());
    [x0, y0, (a[0] + a[2]).ceil() - x0, (a[1] + a[3]).ceil() - y0]
}

fn intersection(a: Area, b: Area) -> Area {
    let (x0, y0) = (a[0].max(b[0]), a[1].max(b[1]));
    let (x1, y1) = ((a[0] + a[2]).min(b[0] + b[2]), (a[1] + a[3]).min(b[1] + b[3]));
    [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
}

/// `patchRect`: the footprint plus three feathers, inside the tile.
pub fn patch_rect(c: &Correction, extent: Area) -> Area {
    let tile = context_rect(c, extent);
    let Some(hole) = stroke_bounding_rect(c, extent) else { return tile };
    let reach = (3.0 * feather(c, 1.0)).ceil();
    intersection(integral([hole[0] - reach, hole[1] - reach, hole[2] + 2.0 * reach, hole[3] + 2.0 * reach]), tile)
}

/// Where a patch of `size` full-resolution pixels lands on `framed` at `scale`: a patch cut to the
/// tile goes on the tile, any other on its patch rectangle. A reduced one covers the whole pixels its
/// exact edges touch, with the shifts (left, bottom) that set its content where the full one lies.
pub fn placed(c: &Correction, size: (u32, u32), framed: Rect, scale: f64) -> (Rect, (f64, f64)) {
    let full = [f64::from(framed.x) / scale, f64::from(framed.y) / scale, f64::from(framed.w) / scale, f64::from(framed.h) / scale];
    let tile = context_rect(c, full);
    let at = if (tile[2], tile[3]) == (f64::from(size.0), f64::from(size.1)) { tile } else { patch_rect(c, full) };
    if scale == 1.0 {
        return (Rect::new(at[0] as i32, at[1] as i32, size.0, size.1), (0.0, 0.0));
    }
    let (left, top, bottom) = (at[0] * scale, at[1] * scale, (at[1] + f64::from(size.1)) * scale);
    let (x0, y1) = ((left + 1e-9).floor(), (bottom - 1e-9).ceil());
    let shift = (left - x0, y1 - bottom);
    let side = |s: u32, shift: f64| ((f64::from(s) * scale + shift - 1e-9).ceil() as u32).max(1);
    let (w, h) = (side(size.0, shift.0), side(size.1, shift.1));
    debug_assert!((y1 - f64::from(h) - top).abs() < 1.0);
    (Rect::new(x0 as i32, (y1 - f64::from(h)) as i32, w, h), shift)
}

/// The stroke's points on `framed`, in its pixels, as the stroke mask's input row.
pub fn stroke_points(c: &Correction, framed: Rect) -> Vec<[f32; 4]> {
    let extent = [f64::from(framed.x), f64::from(framed.y), f64::from(framed.w), f64::from(framed.h)];
    points(c, extent).map(|(x, y)| [x as f32, y as f32, 0.0, 1.0]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::cg::Point;

    #[test]
    fn a_tile_centres_on_its_stroke_and_stays_inside_the_frame() {
        let c = Correction { radius: 10.0, ..Correction::new(vec![Point::new(0.5, 0.5)]) };
        let extent = [0.0, 0.0, 4000.0, 3000.0];
        assert_eq!(context_rect(&c, extent), [1600.0, 1100.0, 800.0, 800.0]);
        let corner = Correction::new(vec![Point::new(0.01, 0.99)]);
        assert_eq!(context_rect(&corner, extent), [0.0, 2200.0, 800.0, 800.0]);
        let p = patch_rect(&c, extent);
        // Radius 10, feather 2, reach 6: the footprint plus the reach, whole pixels.
        assert_eq!(p, [1984.0, 1484.0, 32.0, 32.0]);
    }

    #[test]
    fn a_reduced_patch_lands_where_the_full_one_does() {
        let c = Correction { radius: 10.0, ..Correction::new(vec![Point::new(0.5, 0.5)]) };
        let (full, _) = placed(&c, (32, 32), Rect::new(0, 0, 4000, 3000), 1.0);
        let (half, shift) = placed(&c, (32, 32), Rect::new(0, 0, 2000, 1500), 0.5);
        assert_eq!((full.x, full.y, full.w), (1984, 1484, 32));
        assert_eq!((half.x, half.y, half.w, shift), (992, 742, 16, (0.0, 0.0)));
        // A patch of the tile's size lands on the tile.
        assert_eq!(placed(&c, (800, 800), Rect::new(0, 0, 4000, 3000), 1.0).0, Rect::new(1600, 1100, 800, 800));
    }

    #[test]
    fn a_reduced_patch_reads_its_content_where_the_full_one_lies() {
        use oni_graph::stages::{AnchoredLanczos, Axis};
        // A third of 6048 × 4032, the stroke off any whole pixel of the reduced frame.
        let scale = 2000.0 / 6048.0;
        let framed = Rect::new(0, 0, 2000, 1333);
        let mut worst: f64 = 0.0;
        for (x, y, r) in [(0.1234, 0.5678, 40.0), (0.7771, 0.2093, 120.0), (0.4, 0.91, 7.0)] {
            let c = Correction { radius: r, ..Correction::new(vec![Point::new(x, y), Point::new(x + 0.01, y + 0.003)]) };
            let full = [0.0, 0.0, f64::from(framed.w) / scale, f64::from(framed.h) / scale];
            let at = patch_rect(&c, full);
            let size = (at[2] as u32, at[3] as u32);
            let (tile, shift) = placed(&c, size, framed, scale);
            let along = |axis, from, shift| AnchoredLanczos { axis, origin: 0, from, scale, shift };
            let (sx, sy) = (along(Axis::X, size.0, shift.0), along(Axis::Y, size.1, shift.1));
            assert_eq!((tile.w, tile.h), (sx.to(), sy.to()));
            for o in 0..tile.w as i32 {
                worst = worst.max(((f64::from(tile.x + o) + 0.5) / scale - (at[0] + sx.centre(o) + 0.5)).abs());
            }
            for o in 0..tile.h as i32 {
                worst = worst.max(((f64::from(tile.y + o) + 0.5) / scale - (at[1] + sy.centre(o) + 0.5)).abs());
            }
            // Twin: unshifted, the content sits up to a reduced pixel off.
            let off = (f64::from(tile.x) + 0.5) / scale - (at[0] + along(Axis::X, size.0, 0.0).centre(0) + 0.5);
            assert!(shift.0 == 0.0 || off.abs() > 1e-3, "{off}");
        }
        assert!(worst < 1e-6, "{worst}");
    }
}
