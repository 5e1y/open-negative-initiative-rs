//! The render graph's lens warp: stage 0's turns and angle and a radial map per channel in one read.

use super::*;

/// Each output pixel's centre `c` maps through `t` onto the corrected scan, then each channel reads
/// the scan bilinearly at `centre + P(r²)·d`, `d` its offset from `centre`, `r² = |d|²·inv_unit2`,
/// `P` that channel's polynomial in `r²`; alpha is green's. `transparent` reads zero past the input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Warp {
    pub t: [[f32; 3]; 2],
    pub centre: V2,
    pub inv_unit2: f32,
    /// Red, green, blue: `k0 + k1 r² + k2 r⁴ + k3 r⁶`.
    pub coefficients: [V4; 3],
    pub transparent: bool,
}

impl Warp {
    /// Whether red or blue reads apart from green, three fetches instead of one.
    pub fn fringed(&self) -> bool {
        self.coefficients[0] != self.coefficients[1] || self.coefficients[2] != self.coefficients[1]
    }

    pub fn factor(&self, channel: usize, u: f32) -> f32 {
        let k = self.coefficients[channel];
        k[0] + u * (k[1] + u * (k[2] + u * k[3]))
    }

    pub fn bytes(&self) -> Vec<u8> {
        let head = [u32::from(self.transparent), u32::from(self.fringed()), 0, 0];
        let rows = [
            [self.t[0][0], self.t[0][1], self.t[0][2], 0.0],
            [self.t[1][0], self.t[1][1], self.t[1][2], 0.0],
            [self.centre[0], self.centre[1], self.inv_unit2, 0.0],
            self.coefficients[0],
            self.coefficients[1],
            self.coefficients[2],
        ];
        [bytemuck::bytes_of(&head), bytemuck::cast_slice::<_, u8>(&rows)].concat()
    }
}

fn bilinear(input: &Grid, at: V2, transparent: bool) -> V4 {
    let a = [at[0] - 0.5, at[1] - 0.5];
    let f = [a[0].floor(), a[1].floor()];
    let t = [a[0] - f[0], a[1] - f[1]];
    let q = [f[0] as i32, f[1] as i32];
    let r = |dx: i32, dy: i32| input.read([q[0] + dx, q[1] + dy], transparent);
    mix4(mix4(r(0, 0), r(1, 0), t[0]), mix4(r(0, 1), r(1, 1), t[0]), t[1])
}

/// The warp onto `out`, (x, y, width, height).
pub fn warp(input: &Grid, out: (i32, i32, usize, usize), k: &Warp) -> Grid {
    Grid::generated(out.0, out.1, out.2, out.3, |p| {
        let c = [p[0] as f32 + 0.5, p[1] as f32 + 0.5];
        let s = [k.t[0][0] * c[0] + k.t[0][1] * c[1] + k.t[0][2], k.t[1][0] * c[0] + k.t[1][1] * c[1] + k.t[1][2]];
        let d = [s[0] - k.centre[0], s[1] - k.centre[1]];
        let u = (d[0] * d[0] + d[1] * d[1]) * k.inv_unit2;
        let read = |channel: usize| {
            let f = k.factor(channel, u);
            bilinear(input, [k.centre[0] + f * d[0], k.centre[1] + f * d[1]], k.transparent)
        };
        let g = read(1);
        if !k.fringed() {
            return g;
        }
        [read(0)[0], g[1], read(2)[2], g[3]]
    })
}
