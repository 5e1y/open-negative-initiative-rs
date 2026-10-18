//! The render graph's bilinear affine resampling.

use super::*;

/// An affine resampling: the output pixel centre `c` reads the input at `m · (c.x, c.y, 1)`, both in
/// the graph's pixels, bilinearly; `transparent` reads zero past the input's extent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub m: [[f32; 3]; 2],
    pub transparent: bool,
}

impl Affine {
    pub fn bytes(&self) -> Vec<u8> {
        let rows = [[self.m[0][0], self.m[0][1], self.m[0][2], 0.0], [self.m[1][0], self.m[1][1], self.m[1][2], 0.0]];
        let flags = [u32::from(self.transparent), 0, 0, 0];
        [bytemuck::cast_slice::<_, u8>(&rows), bytemuck::bytes_of(&flags)].concat()
    }
}

/// The bilinear affine resampling onto `out`, (x, y, width, height).
pub fn affine(input: &Grid, out: (i32, i32, usize, usize), k: &crate::Affine) -> Grid {
    Grid::generated(out.0, out.1, out.2, out.3, |p| {
        let c = [p[0] as f32 + 0.5, p[1] as f32 + 0.5];
        let at = [
            k.m[0][0] * c[0] + k.m[0][1] * c[1] + k.m[0][2] - 0.5,
            k.m[1][0] * c[0] + k.m[1][1] * c[1] + k.m[1][2] - 0.5,
        ];
        let f = [at[0].floor(), at[1].floor()];
        let t = [at[0] - f[0], at[1] - f[1]];
        let q = [f[0] as i32, f[1] as i32];
        let r = |dx: i32, dy: i32| input.read([q[0] + dx, q[1] + dy], k.transparent);
        mix4(mix4(r(0, 0), r(1, 0), t[0]), mix4(r(0, 1), r(1, 1), t[0]), t[1])
    })
}
