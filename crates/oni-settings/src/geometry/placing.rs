//! Stage 0's one map between the scan and what it hands on: the turn and mirror table, the fine
//! rotation about the upright box's centre and the frame it cuts, read by the pipeline and the canvas alike.

use super::Size;
use crate::cg::Point;
use crate::frame::Geometry;

/// The turns and mirrors on a `w` × `h` box at the origin: output pixel `p` reads input pixel
/// `a·p + b`, on a box of the returned size.
pub fn orientation(g: &Geometry, (w, h): Size) -> ([i32; 4], [f64; 2], Size) {
    let (a, b, out) = match g.quarter_turns.rem_euclid(4) {
        1 => ([0, -1, 1, 0], [w - 1.0, 0.0], (h, w)),
        2 => ([-1, 0, 0, -1], [w - 1.0, h - 1.0], (w, h)),
        3 => ([0, 1, -1, 0], [0.0, h - 1.0], (h, w)),
        _ => ([1, 0, 0, 1], [0.0, 0.0], (w, h)),
    };
    // A mirror flips the axes seen after the turns: p ↦ F·p + f, composed under the turn.
    let f = [if g.mirrored { -1 } else { 1 }, if g.mirrored_vertically { -1 } else { 1 }];
    let shift = [if g.mirrored { out.0 - 1.0 } else { 0.0 }, if g.mirrored_vertically { out.1 - 1.0 } else { 0.0 }];
    let a2 = [a[0] * f[0], a[1] * f[1], a[2] * f[0], a[3] * f[1]];
    let at = |i: usize| f64::from(a[i]);
    let b2 = [at(0) * shift[0] + at(1) * shift[1] + b[0], at(2) * shift[0] + at(3) * shift[1] + b[1]];
    (a2, b2, out)
}

/// The rotation by `degrees` counter-clockwise about the centre of `upright` (x, y, w, h): the matrix
/// from an output point to the input point, and the bounding box's whole-pixel corners.
pub fn rotation(degrees: f32, upright: [f64; 4]) -> ([[f64; 3]; 2], [f64; 4]) {
    let t = f64::from(degrees).to_radians();
    let (c, s) = (libm::cos(t), libm::sin(t));
    let (cx, cy) = (upright[0] + upright[2] / 2.0, upright[1] + upright[3] / 2.0);
    // On screen, y running down, a positive angle turns counter-clockwise.
    let m = [[c, -s, cx - c * cx + s * cy], [s, c, cy - s * cx - c * cy]];
    let (hw, hh) = ((upright[2] * c.abs() + upright[3] * s.abs()) / 2.0, (upright[2] * s.abs() + upright[3] * c.abs()) / 2.0);
    (m, [(cx - hw).floor(), (cy - hh).floor(), (cx + hw).ceil(), (cy + hh).ceil()])
}

fn outer(r: [f64; 4]) -> [f64; 4] {
    let (x0, y0) = (r[0].floor(), r[1].floor());
    [x0, y0, (r[0] + r[2]).ceil() - x0, (r[1] + r[3]).ceil() - y0]
}

impl Geometry {
    /// The whole-pixel rectangle stage 0 cuts at full resolution, y down, in the upright box's pixels:
    /// the frame, or while the crop is bypassed the rotation's bounding box.
    pub fn frame_extent(&self, image: Size) -> [f64; 4] {
        if !self.crop_bypassed {
            return self.cut(image);
        }
        let (w, h) = self.oriented_size(image);
        if self.angle == 0.0 {
            return [0.0, 0.0, w, h];
        }
        let b = rotation(self.angle, [0.0, 0.0, w, h]).1;
        [b[0], b[1], b[2] - b[0], b[3] - b[1]]
    }

    /// The frame's whole pixels on the upright box, as stage 0 crops them.
    pub(super) fn cut(&self, image: Size) -> [f64; 4] {
        let (w, h) = self.oriented_size(image);
        outer(self.frame_in([0.0, 0.0, w, h], 1.0))
    }

    /// The map between the scan of `image` raw pixels and what stage 0 hands on.
    pub fn placing(&self, image: Size) -> Placing {
        let (a, b, upright) = orientation(self, image);
        // A pixel's index map moved onto points, pixel centres on the half: in = a·out + shift.
        let at = |i: usize| f64::from(a[i]);
        let shift = [b[0] + 0.5 - 0.5 * (at(0) + at(1)), b[1] + 0.5 - 0.5 * (at(2) + at(3))];
        let turn = (self.angle != 0.0).then(|| rotation(self.angle, [0.0, 0.0, upright.0, upright.1]).0);
        Placing { a, shift, turn, image, frame: self.frame_extent(image) }
    }
}

/// Where a point of the scan lies on the frame stage 0 hands on, and back; both sides in fractions,
/// origin top left, y down.
#[derive(Clone, Debug, PartialEq)]
pub struct Placing {
    a: [i32; 4],
    shift: [f64; 2],
    turn: Option<[[f64; 3]; 2]>,
    image: Size,
    frame: [f64; 4],
}

impl Placing {
    pub fn on_frame(&self, scan: Point) -> Point {
        let p = [scan.x * self.image.0 - self.shift[0], scan.y * self.image.1 - self.shift[1]];
        let at = |i: usize| f64::from(self.a[i]);
        // The turn table is a signed permutation: its inverse is its transpose.
        let mut q = [at(0) * p[0] + at(2) * p[1], at(1) * p[0] + at(3) * p[1]];
        if let Some(m) = self.turn {
            let d = [q[0] - m[0][2], q[1] - m[1][2]];
            q = [m[0][0] * d[0] + m[1][0] * d[1], m[0][1] * d[0] + m[1][1] * d[1]];
        }
        let f = self.frame;
        Point::new((q[0] - f[0]) / f[2], (q[1] - f[1]) / f[3])
    }

    pub fn on_scan(&self, framed: Point) -> Point {
        let f = self.frame;
        let mut q = [f[0] + framed.x * f[2], f[1] + framed.y * f[3]];
        if let Some(m) = self.turn {
            q = [m[0][0] * q[0] + m[0][1] * q[1] + m[0][2], m[1][0] * q[0] + m[1][1] * q[1] + m[1][2]];
        }
        let at = |i: usize| f64::from(self.a[i]);
        let p = [at(0) * q[0] + at(1) * q[1] + self.shift[0], at(2) * q[0] + at(3) * q[1] + self.shift[1]];
        Point::new(p[0] / self.image.0, p[1] / self.image.1)
    }
}
