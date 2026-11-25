//! Light through a film's dyes. A reversal film's slide projected on a white screen: its dyes over the base filter a
//! projector's tungsten-halogen lamp, and the eye, adapted to the bare film's light on the screen, sees the rest with the
//! room's veil laid over. A scanned slide is read back to those dyes first, its light under the scanner's lamp solved for
//! their amounts. A colour negative's scan, by ONI's own scanner, shares the kernel and its table of dyes.

use super::{bake, cie, profile::Profile};
use crate::baked::Baked;
use oni_render::{ColorMatrix, DyeAmounts, Projection, cpu};
use oni_settings::VirtualStock;
use oni_settings::stocks::VirtualEntry;

type M3 = [[f64; 3]; 3];
type V3 = [f32; 3];

/// A slide projector's lamp, a tungsten-halogen bulb.
pub const LAMP_KELVIN: f64 = 3200.0;
/// A slide scanner's backlight, a daylight lamp: the decode's white balance reads it near neutral.
pub const SCAN_KELVIN: f64 = 5500.0;
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
    /// The film's densest, each layer's most along its curve, as the screen shows it: a slide's unexposed black.
    pub black: V3,
    /// Its film's file: a projection made of another is never served for it.
    pub revision: u64,
}

/// A slide's scan read back to its dyes: the solve at no gain, its table of the dyes and the scanner's lamp as the
/// observer sees it, and that lamp's light through no film in working Rec. 2020, which a frame's gains scale.
pub struct Scanned {
    pub kernel: DyeAmounts,
    pub table: Vec<[f32; 4]>,
    pub open: V3,
    pub revision: u64,
}

impl Scanned {
    /// The solve for a frame whose gains, in stops, put the backlight on 1.
    pub fn at(&self, stops: V3) -> DyeAmounts {
        DyeAmounts { gain: std::array::from_fn(|c| stops[c].exp2() * self.open[c]), ..self.kernel.clone() }
    }
}

/// A colour negative's scan: its dyes and base, then the scanner's lamp as the camera sees it, two rows of the bands.
#[derive(Clone, Debug, PartialEq)]
pub struct Scan {
    pub projection: Projection,
    pub table: Vec<[f32; 4]>,
}

/// `stock`'s slide projected, built at its first use; a film's file edited builds it again.
pub fn projected(stock: VirtualStock) -> &'static Projected {
    static PROJECTED: Baked<(VirtualStock, u64), Projected> = Baked::new();
    let entry = stock.entry();
    PROJECTED.get((entry.stock, entry.revision), || project(&entry))
}

/// `stock`'s scans read back to its dyes, built at its first use; a film's file edited builds it again.
pub fn scanned(stock: VirtualStock) -> &'static Scanned {
    static SCANNED: Baked<(VirtualStock, u64), Scanned> = Baked::new();
    let entry = stock.entry();
    SCANNED.get((entry.stock, entry.revision), || scan(&entry))
}

/// Each band's values where the sheet measures them, a band it leaves unmeasured holding its nearest measured neighbours'
/// line: the sheets stop short of 380 and 780 nm, and a dye read as clear there let the lamp's ends through every black.
fn held(values: &[Option<f64>]) -> Vec<f64> {
    let known: Vec<(usize, f64)> = values.iter().enumerate().filter_map(|(i, v)| v.map(|v| (i, v))).collect();
    (0..values.len())
        .map(|i| match known.partition_point(|(at, _)| *at < i) {
            _ if known.is_empty() => 0.0,
            0 => known[0].1,
            k if k == known.len() => known[k - 1].1,
            k => {
                let ((i0, v0), (i1, v1)) = (known[k - 1], known[k]);
                v0 + (v1 - v0) * (i - i0) as f64 / (i1 - i0) as f64
            }
        })
        .collect()
}

/// A film's dyes at unit amount and its base, band by band, every band held.
pub(super) fn dyes(profile: &Profile) -> (Vec<[f64; 3]>, Vec<f64>) {
    let columns: [Vec<f64>; 3] = std::array::from_fn(|c| held(&profile.channel_density.iter().map(|d| d[c]).collect::<Vec<_>>()));
    let dyes = (0..cie::WAVELENGTHS).map(|i| std::array::from_fn(|c| columns[c][i])).collect();
    (dyes, held(&profile.base_density))
}

/// The lamp at `kelvin` as the observer sees it band by band, and the matrix taking that light, adapted by Bradford from
/// the bare film's to D65 at a luminance of one, to working Rec. 2020: the bare film reads (1, 1, 1).
fn seen(kelvin: f64, base: &[f64]) -> (Vec<[f64; 3]>, M3) {
    let at = |i: usize| cie::FIRST_NM + cie::STEP_NM * i as f64;
    let seen: Vec<[f64; 3]> = (0..cie::WAVELENGTHS).map(|i| [cie::X[i], cie::Y[i], cie::Z[i]].map(|v| v * planck(at(i), kelvin))).collect();
    let white: [f64; 3] = std::array::from_fn(|c| (0..cie::WAVELENGTHS).map(|i| 10f64.powf(-base[i]) * seen[i][c]).sum());
    let (cone_w, cone_d) = (mul(&BRADFORD, white), mul(&BRADFORD, D65));
    let gain: M3 = std::array::from_fn(|i| std::array::from_fn(|j| if i == j { cone_d[i] / cone_w[i] } else { 0.0 }));
    (seen, matmul(&inverse(&REC2020), &matmul(&inverse(&BRADFORD), &matmul(&gain, &BRADFORD))))
}

/// The dyes and the base, then the lamp as seen, as the projection kernel reads them.
pub(super) fn tabled(dyes: &[[f64; 3]], base: &[f64], seen: &[[f64; 3]]) -> Vec<[f32; 4]> {
    let n = cie::WAVELENGTHS;
    let mut table = vec![[0.0f32; 4]; 2 * n];
    for i in 0..n {
        table[i] = [dyes[i][0] as f32, dyes[i][1] as f32, dyes[i][2] as f32, base[i] as f32];
        table[n + i] = [seen[i][0] as f32, seen[i][1] as f32, seen[i][2] as f32, 0.0];
    }
    table
}

fn read(entry: &VirtualEntry) -> Profile {
    Profile::read(entry.profile.as_bytes()).expect("a stock's profile reads once its file is taken")
}

fn project(entry: &VirtualEntry) -> Projected {
    let profile = read(entry);
    let (dyes, base) = dyes(&profile);
    let (seen, to_working) = seen(LAMP_KELVIN, &base);
    let table = tabled(&dyes, &base, &seen);
    let projection = Projection { to_working: to_working.map(|r| r.map(|v| v as f32)), flare: FLARE, bands: cie::WAVELENGTHS };
    let densest: V3 = std::array::from_fn(|c| profile.density_curves.iter().map(|d| d[c]).fold(f64::NEG_INFINITY, f64::max) as f32);
    let shown = cpu::projection([10f32.powf(-densest[0]), 10f32.powf(-densest[1]), 10f32.powf(-densest[2]), 1.0], &projection, &table);
    Projected { projection, table, black: [shown[0], shown[1], shown[2]], revision: entry.revision }
}

fn scan(entry: &VirtualEntry) -> Scanned {
    let profile = read(entry);
    let (dyes, base) = dyes(&profile);
    let (seen, to_working) = seen(SCAN_KELVIN, &base);
    let n = cie::WAVELENGTHS;
    // The solve reads the observer's light, each function scaled so the bare film reads 1, as working Rec. 2020 does.
    let white: [f64; 3] = std::array::from_fn(|c| (0..n).map(|i| 10f64.powf(-base[i]) * seen[i][c]).sum());
    let observed: Vec<[f64; 3]> = seen.iter().map(|s| std::array::from_fn(|c| s[c] / white[c])).collect();
    let back = inverse(&to_working);
    let to_seen: [V3; 3] = std::array::from_fn(|r| std::array::from_fn(|c| (back[r][c] / white[r]) as f32));
    let table = tabled(&dyes, &base, &observed);
    let light = |amounts: [f64; 3]| -> [f64; 3] {
        let t = |i: usize| 10f64.powf(-(base[i] + (0..3).map(|k| amounts[k] * dyes[i][k]).sum::<f64>()));
        std::array::from_fn(|c| (0..n).map(|i| t(i) * observed[i][c]).sum())
    };
    let open: [f64; 3] = std::array::from_fn(|c| seen.iter().map(|s| mul(&to_working, *s)[c]).sum());
    // Each dye alone at unit amount, its density over the bare film on each of the observer's: the guess inverts that.
    let cross: M3 = std::array::from_fn(|k| light(std::array::from_fn(|m| f64::from(u8::from(m == k)))).map(|v| -v.max(1e-9).log10()));
    let inverted = inverse(&cross);
    let start: [V3; 3] = std::array::from_fn(|r| std::array::from_fn(|c| inverted[c][r] as f32));
    Scanned { kernel: DyeAmounts { gain: [1.0; 3], to_seen, start, bands: n }, table, open: open.map(|v| v as f32), revision: entry.revision }
}

/// The neutral densities a slide's Lighting reads its layers on, and each layer's amount a neutral of that density holds
/// under the scanner's lamp, rising until the film's densest.
pub const NEUTRAL_HIGH: f64 = 4.0;
pub const NEUTRAL_NODES: usize = 401;

/// `stock`'s tone as Lighting develops a slide again: along its sheet's log exposure, each layer's amount where the
/// green layer's curve lays a neutral, so every layer reads the one neutral density and a grey re-exposed stays grey.
pub fn tone(stock: VirtualStock) -> (Vec<f64>, Vec<[f64; 3]>) {
    let entry = stock.entry();
    let profile = read(&entry);
    let s = scanned(stock);
    let density: Vec<f64> = (0..NEUTRAL_NODES).map(|i| NEUTRAL_HIGH * i as f64 / (NEUTRAL_NODES - 1) as f64).collect();
    let mut held = [f64::NEG_INFINITY; 3];
    let neutral: Vec<[f64; 3]> = density
        .iter()
        .map(|d| {
            let a = cpu::amounts([10f32.powf(-*d as f32); 3], &s.kernel, &s.table);
            held = std::array::from_fn(|c| held[c].max(f64::from(a[c])));
            held
        })
        .collect();
    let green: Vec<f64> = neutral.iter().map(|a| a[1]).collect();
    let densities: Vec<[f64; 3]> = density.iter().map(|d| [*d; 3]).collect();
    let amounts = profile
        .density_curves
        .iter()
        .map(|curve| {
            let d = bake::interpolate(curve[1], &green, &densities, 0);
            std::array::from_fn(|c| bake::interpolate(d, &density, &neutral, c))
        })
        .collect();
    (profile.log_exposure, amounts)
}

/// The screen's black laid on pure black and the bare film's white on pure white, each channel alone: Projected blacks
/// off, for a slide shown on a screen.
pub fn pure(p: &Projected) -> ColorMatrix {
    let white = 1.0 + p.projection.flare;
    let scale: V3 = std::array::from_fn(|c| 1.0 / (white - p.black[c]));
    ColorMatrix {
        r_vector: [scale[0], 0.0, 0.0, 0.0],
        g_vector: [0.0, scale[1], 0.0, 0.0],
        b_vector: [0.0, 0.0, scale[2], 0.0],
        bias_vector: [-p.black[0] * scale[0], -p.black[1] * scale[1], -p.black[2] * scale[2], 0.0],
        ..ColorMatrix::default()
    }
}

/// Planck's law at `nm` for a body at `kelvin`, to a constant factor.
pub(super) fn planck(nm: f64, kelvin: f64) -> f64 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unmeasured_band_holds_its_nearest_measured_neighbours() {
        assert_eq!(held(&[None, Some(1.0), None, Some(3.0), None]), vec![1.0, 1.0, 2.0, 3.0, 3.0]);
        // Twin: read as clear, the ends would be none.
        assert_ne!(held(&[None, Some(1.0)])[0], 0.0);
    }
}
