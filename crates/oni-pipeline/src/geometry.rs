//! Stage 0, `Pipeline.applyGeometry`: quarter turns, then mirrors, then the fine rotation about the
//! centre, then the crop quantised on the full-resolution grid, in the graph's y-down pixels.

use crate::stages::{OpaqueEdge, Oriented, Resample};
use oni_graph::stages::Crop;
use oni_graph::{Image, Rect};
use oni_render::{Affine, Orient};
use oni_settings::frame::Geometry;

/// The turns and mirrors as one permutation from `input` onto an extent at the origin.
pub fn orientation(g: &Geometry, input: Rect) -> (Orient, Rect) {
    let (w, h) = (input.w as i32, input.h as i32);
    let (a, b, out_w, out_h) = match g.quarter_turns.rem_euclid(4) {
        1 => ([0, -1, 1, 0], [w - 1, 0], h, w),
        2 => ([-1, 0, 0, -1], [w - 1, h - 1], w, h),
        3 => ([0, 1, -1, 0], [0, h - 1], h, w),
        _ => ([1, 0, 0, 1], [0, 0], w, h),
    };
    // A mirror flips the axes seen after the turns: p ↦ F·p + f, composed under the turn.
    let f = [if g.mirrored { -1 } else { 1 }, if g.mirrored_vertically { -1 } else { 1 }];
    let shift = [if g.mirrored { out_w - 1 } else { 0 }, if g.mirrored_vertically { out_h - 1 } else { 0 }];
    let a2 = [a[0] * f[0], a[1] * f[1], a[2] * f[0], a[3] * f[1]];
    let b2 = [
        a[0] * shift[0] + a[1] * shift[1] + b[0] + input.x,
        a[2] * shift[0] + a[3] * shift[1] + b[1] + input.y,
    ];
    (Orient { a: a2, b: b2 }, Rect::new(0, 0, out_w as u32, out_h as u32))
}

/// The rotation by `degrees` counter-clockwise about `upright`'s centre, onto its bounding box.
pub fn rotation(degrees: f32, upright: Rect) -> (Affine, Rect) {
    let t = f64::from(degrees).to_radians();
    let (c, s) = (t.cos(), t.sin());
    let cx = f64::from(upright.x) + f64::from(upright.w) / 2.0;
    let cy = f64::from(upright.y) + f64::from(upright.h) / 2.0;
    // Output centre to input point; on screen, y running down, a positive angle turns counter-clockwise.
    let m = [[c, -s, cx - c * cx + s * cy], [s, c, cy - s * cx - c * cy]];
    let (hw, hh) = (
        (f64::from(upright.w) * c.abs() + f64::from(upright.h) * s.abs()) / 2.0,
        (f64::from(upright.w) * s.abs() + f64::from(upright.h) * c.abs()) / 2.0,
    );
    let extent = Rect::from_corners(
        (cx - hw).floor() as i32,
        (cy - hh).floor() as i32,
        (cx + hw).ceil() as i32,
        (cy + hh).ceil() as i32,
    );
    (Affine { m: m.map(|r| r.map(|v| v as f32)), transparent: true }, extent)
}

/// The frame in the upright box's pixels, before rounding: fractions with a bottom-left origin, as
/// every readout of the kept size reads them.
pub fn frame(g: &Geometry, upright: Rect, scale: f64) -> [f64; 4] {
    g.frame_in([f64::from(upright.x), f64::from(upright.y), f64::from(upright.w), f64::from(upright.h)], scale)
}

fn integral(v: f64) -> bool {
    (v - v.round()).abs() < 1e-6
}

/// Stage 0's steps on an input of extent `input`, each with the extent it leaves: the render and the
/// size it is shown at read one plan.
struct Cuts {
    oriented: Option<(Orient, Rect)>,
    rotated: Option<(Affine, Rect)>,
    /// The crop's outer frame, and whether the frame lies between pixels.
    cropped: Option<(Rect, bool)>,
    extent: Rect,
}

fn cuts(g: &Geometry, input: Rect, scale: f64) -> Cuts {
    let mut c = Cuts { oriented: None, rotated: None, cropped: None, extent: input };
    if g.is_neutral() {
        return c;
    }
    if g.quarter_turns.rem_euclid(4) != 0 || g.mirrored || g.mirrored_vertically {
        let o = orientation(g, input);
        c.extent = o.1;
        c.oriented = Some(o);
    }
    let upright = c.extent;
    if g.angle != 0.0 {
        let r = rotation(g.angle, upright);
        c.extent = r.1;
        c.rotated = Some(r);
    }
    if !g.crop_bypassed {
        let r = frame(g, upright, scale);
        let (x0, y0, x1, y1) = (r[0], r[1], r[0] + r[2], r[1] + r[3]);
        let outer = Rect::from_corners(x0.floor() as i32, y0.floor() as i32, x1.ceil() as i32, y1.ceil() as i32);
        c.extent = c.extent.intersect(&outer);
        c.cropped = Some((outer, ![x0, y0, x1, y1].into_iter().all(integral)));
    }
    c
}

/// The extent stage 0 leaves of an input of extent `input`, as `apply` cuts it.
pub fn extent(g: &Geometry, input: Rect, scale: f64) -> Rect {
    cuts(g, input, scale).extent
}

/// Stage 0 on `image`; `scale` is its width over the full-resolution width, so every scale crops one frame.
pub fn apply(image: Image, g: &Geometry, scale: f64) -> Image {
    let c = cuts(g, image.extent(), scale);
    let mut out = image;
    if let Some((k, extent)) = c.oriented {
        out = out.then(Oriented { k, extent });
    }
    if let Some((k, extent)) = c.rotated {
        out = out.then(Resample { k, extent });
    }
    if let Some((outer, between)) = c.cropped {
        out = out.then(Crop(outer));
        // A frame between pixels has its border partly covered: the line inside it stands in.
        if between && outer.w > 2 && outer.h > 2 {
            let extent = out.extent();
            out = out.then(OpaqueEdge { inner: extent.outset(-1, -1), extent });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::cg;

    fn turned(k: i64, mirrored: bool) -> Geometry {
        Geometry { quarter_turns: k, mirrored, ..Geometry::default() }
    }

    fn at(o: &Orient, p: [i32; 2]) -> [i32; 2] {
        [o.a[0] * p[0] + o.a[1] * p[1] + o.b[0], o.a[2] * p[0] + o.a[3] * p[1] + o.b[1]]
    }

    #[test]
    fn a_quarter_turn_takes_the_top_right_corner_to_the_top_left() {
        let input = Rect::new(0, 0, 6, 4);
        let (k, extent) = orientation(&turned(1, false), input);
        assert_eq!(extent, Rect::new(0, 0, 4, 6));
        assert_eq!(at(&k, [0, 0]), [5, 0]);
        assert_eq!(at(&k, [3, 5]), [0, 3]);
        // Twin: three quarters is the other way round, top left from bottom left.
        let (cw, _) = orientation(&turned(3, false), input);
        assert_eq!(at(&cw, [0, 0]), [0, 3]);
        // The mirror flips the turned picture's own axis.
        let (m, _) = orientation(&turned(1, true), input);
        assert_eq!(at(&m, [0, 0]), at(&k, [3, 0]));
        assert_eq!(orientation(&turned(-1, false), input).0, cw);
    }

    #[test]
    fn the_crop_reads_bottom_left_fractions_on_the_full_grid() {
        let g = Geometry { crop: cg::Rect::new(0.1, 0.2, 0.5, 0.25), ..Geometry::default() };
        let r = frame(&g, Rect::new(0, 0, 1000, 800), 1.0);
        assert_eq!(r, [100.0, (1.0 - 0.45) * 800.0, 500.0, 200.0]);
        // A reduced copy quantises on the full grid: the fraction is the full frame's, not the copy's.
        let fine = Geometry { crop: cg::Rect::new(0.1003, 0.0, 1.0 - 0.1003, 1.0), ..Geometry::default() };
        let full = frame(&fine, Rect::new(0, 0, 10000, 8000), 1.0);
        let reduced = frame(&fine, Rect::new(0, 0, 1000, 800), 0.1);
        assert!((reduced[0] * 10.0 - full[0]).abs() < 1e-6, "{} against {}", reduced[0], full[0]);
        // Twin: quantised on the copy's own grid, the frame lands a pixel away.
        let own = frame(&fine, Rect::new(0, 0, 1000, 800), 1.0);
        assert!((own[0] * 10.0 - full[0]).abs() > 1.0);
    }

    #[test]
    fn the_rotation_keeps_the_centre_and_covers_its_bounding_box() {
        let upright = Rect::new(0, 0, 100, 60);
        let (k, extent) = rotation(5.0, upright);
        let c = [50.0f32, 30.0];
        let back = [k.m[0][0] * c[0] + k.m[0][1] * c[1] + k.m[0][2], k.m[1][0] * c[0] + k.m[1][1] * c[1] + k.m[1][2]];
        assert!((back[0] - 50.0).abs() < 1e-4 && (back[1] - 30.0).abs() < 1e-4);
        assert!(extent.contains(&upright) && extent.w > 100 && extent.h > 60);
        // A point right of the centre comes from below it, on screen, for a counter-clockwise turn.
        let p = [60.0f32, 30.0];
        assert!(k.m[1][0] * p[0] + k.m[1][1] * p[1] + k.m[1][2] > 30.0);
    }

    /// The view's 1:1 size: a turned frame cut off the preview's grid, the preview reduced a quarter.
    fn off_grid() -> Geometry {
        Geometry { quarter_turns: 1, crop: cg::Rect::new(0.1003, 0.0, 0.8997, 1.0), ..Geometry::default() }
    }

    #[test]
    fn the_full_frames_size_is_read_on_its_own_grid_never_scaled_up_from_the_previews() {
        let (full, preview) = (Rect::new(0, 0, 1600, 1000), Rect::new(0, 0, 400, 250));
        assert_eq!(extent(&off_grid(), full, 1.0), Rect::new(101, 0, 899, 1600));
        // Twin: the preview's own cut, scaled up, says a column more than the full frame holds.
        let guessed = extent(&off_grid(), preview, 0.25);
        assert_eq!(guessed.w * 4, 900);
        // A tilt's bleed is a pixel of the grid it is cut on: four full pixels on a quarter preview.
        let tilted = Geometry { angle: 1.0, ..off_grid() };
        assert_ne!(extent(&tilted, preview, 0.25).w * 4, extent(&tilted, full, 1.0).w);
    }

    #[test]
    fn the_extent_read_is_the_one_stage_0_cuts() {
        let Ok(mut graph) = oni_graph::Graph::new() else {
            return eprintln!("no adapter: stage 0's extent is not compared");
        };
        let (w, h) = (1600, 1000);
        let source = graph.upload(w, h, &vec![[0.5; 4]; (w * h) as usize]).unwrap();
        let input = Rect::new(0, 0, w, h);
        let crop = cg::Rect::new(0.1003, 0.2, 0.5, 0.4567);
        let geometries = [
            Geometry::default(),
            Geometry { crop, ..Geometry::default() },
            off_grid(),
            Geometry { quarter_turns: 3, mirrored: true, crop, ..Geometry::default() },
            Geometry { angle: -2.5, crop, ..Geometry::default() },
            Geometry { angle: 1.0, quarter_turns: 1, crop, crop_bypassed: true, ..Geometry::default() },
        ];
        for g in &geometries {
            for scale in [1.0, 0.25] {
                assert_eq!(extent(g, input, scale), apply(Image::new(&source), g, scale).extent(), "{g:?} at {scale}");
            }
        }
        // Twin: the input's own extent, which a neutral frame keeps, is not what a cropped one leaves.
        assert_ne!(input, apply(Image::new(&source), &geometries[1], 1.0).extent());
    }
}
