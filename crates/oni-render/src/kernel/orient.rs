//! The render graph's quarter turns and mirrors.

use super::*;

/// An exact permutation: output pixel `p` takes input pixel `a · p + b`, `a` in rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Orient {
    pub a: [i32; 4],
    pub b: [i32; 2],
}

impl Orient {
    pub fn bytes(&self) -> Vec<u8> {
        [bytemuck::bytes_of(&self.a), bytemuck::bytes_of(&[self.b[0], self.b[1], 0, 0])].concat()
    }
}

/// The permutation onto `out`, (x, y, width, height).
pub fn orient(input: &Grid, out: (i32, i32, usize, usize), k: &crate::Orient) -> Grid {
    Grid::generated(out.0, out.1, out.2, out.3, |p| {
        input.load([k.a[0] * p[0] + k.a[1] * p[1] + k.b[0], k.a[2] * p[0] + k.a[3] * p[1] + k.b[1]])
    })
}
