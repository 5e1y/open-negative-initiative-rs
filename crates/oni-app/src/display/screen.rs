//! The screens the canvas is proofed to, and their matrices from linear Rec. 2020.

/// The screen the canvas is proofed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Srgb,
    DisplayP3,
}

pub(super) type M3 = [[f64; 3]; 3];

/// CIE xy of the red, green and blue primaries; every space here sits on D65.
pub(super) const REC2020: [[f64; 2]; 3] = [[0.708, 0.292], [0.170, 0.797], [0.131, 0.046]];
pub(super) const SRGB: [[f64; 2]; 3] = [[0.64, 0.33], [0.30, 0.60], [0.15, 0.06]];
pub(super) const P3: [[f64; 2]; 3] = [[0.680, 0.320], [0.265, 0.690], [0.150, 0.060]];
pub(super) const D65: [f64; 2] = [0.3127, 0.3290];

pub(super) fn mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

pub(super) fn inverse(m: &M3) -> M3 {
    let c = |r: usize, s: usize| {
        let (r0, r1) = ((r + 1) % 3, (r + 2) % 3);
        let (s0, s1) = ((s + 1) % 3, (s + 2) % 3);
        m[r0][s0] * m[r1][s1] - m[r0][s1] * m[r1][s0]
    };
    let det: f64 = (0..3).map(|j| m[0][j] * c(0, j)).sum();
    std::array::from_fn(|i| std::array::from_fn(|j| c(j, i) / det))
}

/// The RGB to XYZ matrix of a space, its white mapping to Y = 1.
pub(super) fn to_xyz(primaries: &[[f64; 2]; 3]) -> M3 {
    let xyz = |p: [f64; 2]| [p[0] / p[1], 1.0, (1.0 - p[0] - p[1]) / p[1]];
    let columns = primaries.map(xyz);
    let m: M3 = std::array::from_fn(|i| std::array::from_fn(|j| columns[j][i]));
    let white = xyz(D65);
    let inv = inverse(&m);
    let s: [f64; 3] = std::array::from_fn(|i| (0..3).map(|k| inv[i][k] * white[k]).sum());
    std::array::from_fn(|i| std::array::from_fn(|j| m[i][j] * s[j]))
}

impl Screen {
    /// Linear Rec. 2020 to this screen's linear primaries.
    pub fn matrix(self) -> [[f32; 3]; 3] {
        let target = match self {
            Screen::Srgb => &SRGB,
            Screen::DisplayP3 => &P3,
        };
        mul(&inverse(&to_xyz(target)), &to_xyz(&REC2020)).map(|r| r.map(|v| v as f32))
    }
}
