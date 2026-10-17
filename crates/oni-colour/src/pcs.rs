//! The profile connection space: D50 XYZ, CIELAB against it, and the 3×3 algebra the legs need.

/// The PCS white every relative colorimetric conversion lands a white on.
pub const PCS_WHITE: [f32; 3] = [0.9642, 1.0, 0.8249];

pub type Mat3 = [[f64; 3]; 3];

pub fn mul(m: &Mat3, v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

pub fn mul_f32(m: &Mat3, v: [f32; 3]) -> [f32; 3] {
    let o = mul(m, [v[0] as f64, v[1] as f64, v[2] as f64]);
    [o[0] as f32, o[1] as f32, o[2] as f32]
}

pub fn matmul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

pub fn inverse(m: &Mat3) -> Option<Mat3> {
    let c = |r: usize, s: usize, t: usize, u: usize| m[r][s] * m[t][u] - m[r][u] * m[t][s];
    let det = m[0][0] * c(1, 1, 2, 2) - m[0][1] * c(1, 0, 2, 2) + m[0][2] * c(1, 0, 2, 1);
    if det.abs() < 1e-12 {
        return None;
    }
    let adj = [
        [c(1, 1, 2, 2), -c(0, 1, 2, 2), c(0, 1, 1, 2)],
        [-c(1, 0, 2, 2), c(0, 0, 2, 2), -c(0, 0, 1, 2)],
        [c(1, 0, 2, 1), -c(0, 0, 2, 1), c(0, 0, 1, 1)],
    ];
    Some(adj.map(|row| row.map(|v| v / det)))
}

/// CIELAB against the PCS white.
pub fn lab(xyz: [f32; 3]) -> [f32; 3] {
    let f = |v: f32| {
        if v > 216.0 / 24389.0 { v.cbrt() } else { (24389.0 / 27.0 * v + 16.0) / 116.0 }
    };
    let (fx, fy, fz) = (f(xyz[0] / PCS_WHITE[0]), f(xyz[1] / PCS_WHITE[1]), f(xyz[2] / PCS_WHITE[2]));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

pub fn xyz_from_lab(lab: [f32; 3]) -> [f32; 3] {
    let fy = (lab[0] + 16.0) / 116.0;
    let (fx, fz) = (fy + lab[1] / 500.0, fy - lab[2] / 200.0);
    let g = |f: f32| {
        if f * f * f > 216.0 / 24389.0 { f * f * f } else { (116.0 * f - 16.0) / (24389.0 / 27.0) }
    };
    [g(fx) * PCS_WHITE[0], g(fy) * PCS_WHITE[1], g(fz) * PCS_WHITE[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lab_round_trips_and_its_twin_does_not() {
        for xyz in [[0.2, 0.3, 0.1], [0.001, 0.002, 0.003], PCS_WHITE] {
            let back = xyz_from_lab(lab(xyz));
            assert!((0..3).all(|i| (back[i] - xyz[i]).abs() < 1e-5), "{xyz:?} -> {back:?}");
        }
        assert!(lab(PCS_WHITE)[1].abs() < 1e-4 && lab(PCS_WHITE)[2].abs() < 1e-4);
        // Twin: read against D65 instead, the PCS white shows a chroma.
        let d65 = [0.9505f32, 1.0, 1.089];
        let wrong = 200.0 * (1.0 - (PCS_WHITE[2] / d65[2]).cbrt());
        assert!(wrong.abs() > 1.0);
    }

    #[test]
    fn inverse_inverts() {
        let m = [[0.6, 0.2, 0.1], [0.3, 0.6, 0.1], [0.0, 0.1, 0.8]];
        let i = inverse(&m).unwrap();
        let id = matmul(&m, &i);
        for (r, row) in id.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                assert!((v - if r == c { 1.0 } else { 0.0 }).abs() < 1e-12);
            }
        }
        // Twin: a singular matrix has none.
        assert!(inverse(&[[1.0, 2.0, 3.0], [2.0, 4.0, 6.0], [0.0, 1.0, 0.0]]).is_none());
    }
}
