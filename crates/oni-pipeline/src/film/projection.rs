//! A reversal film's slide projected on a white screen: its dyes over the base filter a projector's tungsten-halogen
//! lamp, and the eye, adapted to the bare film's light on the screen, sees the rest with the room's veil laid over.

use super::{cie, profile::Profile};
use crate::baked::Baked;
use oni_render::Projection;
use oni_settings::VirtualStock;
use oni_settings::stocks::VirtualEntry;

type M3 = [[f64; 3]; 3];

/// A slide projector's lamp, a tungsten-halogen bulb.
pub const LAMP_KELVIN: f64 = 3200.0;
/// The screen's veil: the room's and the lens's stray light, a share of the screen's white over every tone.
pub const FLARE: f32 = 0.005;

const BRADFORD: M3 = [[0.8951, 0.2664, -0.1614], [-0.7502, 1.7135, 0.0367], [0.0389, -0.0685, 1.0296]];
const D65: [f64; 3] = [0.95047, 1.0, 1.08883];
/// Linear Rec. 2020 to XYZ on its D65 white.
const REC2020: M3 = [[0.6369580, 0.1446169, 0.1688810], [0.2627002, 0.6779981, 0.0593017], [0.0, 0.0280727, 1.0609851]];

/// A slide's projection and its table, two rows of the spectral bands: the dyes at unit amount and the base, then the
/// lamp as the 1931 observer sees it.
pub struct Projected {
    pub projection: Projection,
    pub table: Vec<[f32; 4]>,
    /// Its film's file: a projection made of another is never served for it.
    pub revision: u64,
}

/// `stock`'s slide projected, built at its first use; a film's file edited builds it again.
pub fn projected(stock: VirtualStock) -> &'static Projected {
    static PROJECTED: Baked<(VirtualStock, u64), Projected> = Baked::new();
    let entry = stock.entry();
    PROJECTED.get((entry.stock, entry.revision), || build(&entry))
}

fn build(entry: &VirtualEntry) -> Projected {
    let profile = Profile::read(entry.profile.as_bytes()).expect("a stock's profile reads once its file is taken");
    let n = cie::WAVELENGTHS;
    let at = |i: usize| cie::FIRST_NM + cie::STEP_NM * i as f64;
    let lamp: Vec<f64> = (0..n).map(|i| planck(at(i), LAMP_KELVIN)).collect();
    let base = |i: usize| profile.base_density[i].unwrap_or(0.0);
    let mut table = vec![[0.0f32; 4]; 2 * n];
    let mut white = [0.0; 3];
    for i in 0..n {
        let dye = profile.channel_density[i].map(|d| d.unwrap_or(0.0));
        table[i] = [dye[0] as f32, dye[1] as f32, dye[2] as f32, base(i) as f32];
        let seen = [lamp[i] * cie::X[i], lamp[i] * cie::Y[i], lamp[i] * cie::Z[i]];
        table[n + i] = [seen[0] as f32, seen[1] as f32, seen[2] as f32, 0.0];
        let bare = 10f64.powf(-base(i));
        white = std::array::from_fn(|c| white[c] + bare * seen[c]);
    }
    // The bare film's light, adapted by Bradford onto D65 at a luminance of one, reads (1, 1, 1) in Rec. 2020.
    let (cone_w, cone_d) = (mul(&BRADFORD, white), mul(&BRADFORD, D65));
    let gain: M3 = std::array::from_fn(|i| std::array::from_fn(|j| if i == j { cone_d[i] / cone_w[i] } else { 0.0 }));
    let to_working = matmul(&inverse(&REC2020), &matmul(&inverse(&BRADFORD), &matmul(&gain, &BRADFORD)));
    let projection = Projection { to_working: to_working.map(|r| r.map(|v| v as f32)), flare: FLARE, bands: n };
    Projected { projection, table, revision: entry.revision }
}

/// Planck's law at `nm` for a body at `kelvin`, to a constant factor.
fn planck(nm: f64, kelvin: f64) -> f64 {
    let m = nm * 1e-9;
    1.0 / (m.powi(5) * ((1.438_776_877e-2 / (m * kelvin)).exp() - 1.0))
}

fn mul(m: &M3, v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

fn matmul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn inverse(m: &M3) -> M3 {
    let c = |a: usize, b: usize, e: usize, f: usize| m[a][b] * m[e][f] - m[a][f] * m[e][b];
    let d = m[0][0] * c(1, 1, 2, 2) - m[0][1] * c(1, 0, 2, 2) + m[0][2] * c(1, 0, 2, 1);
    [
        [c(1, 1, 2, 2) / d, -c(0, 1, 2, 2) / d, c(0, 1, 1, 2) / d],
        [-c(1, 0, 2, 2) / d, c(0, 0, 2, 2) / d, -c(0, 0, 1, 2) / d],
        [c(1, 0, 2, 1) / d, -c(0, 0, 2, 1) / d, c(0, 0, 1, 1) / d],
    ]
}
