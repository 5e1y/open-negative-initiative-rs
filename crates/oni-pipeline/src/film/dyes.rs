//! Each dye set as the scan's camera reads it, from the profile its stock's file holds: the light table's D50 read
//! by the 1931 observer in Rec. 2020, the virtual negative's own scan. The generic set is its sources' mean at each
//! wavelength most of them measure.

use super::bake::{self, M3};
use super::cie;
use super::profile::Profile;
use crate::baked::Baked;
use oni_render::DyeSeparation;
use oni_settings::DyeSet;
use oni_settings::stocks::DyeSource;

/// A dye set read by the camera: the dyes and the weights the kernel sums, the bare film, the matrix.
#[derive(Debug)]
pub struct Dyes {
    pub dyes: Vec<[f32; 4]>,
    /// The camera's own channels, the 1931 observer under the light table's D50, before its matrix.
    pub weights: Vec<[f32; 3]>,
    /// The bare film as the scan reads it, in Rec. 2020 light.
    pub bare: [f32; 3],
    /// Rec. 2020 to the camera's own channels.
    pub to_camera: [[f32; 3]; 3],
    /// The bare film's density on the camera's channels.
    pub base: [f32; 3],
    /// Camera densities above the base to dye amounts, by rows: the inverse of the camera's response to each dye
    /// at a unit of every dye, near a negative's mid-grey.
    pub matrix: [[f32; 3]; 3],
}

/// The colour profiles a mean set averages, the generic set's the nine still colour negatives most shot today.
pub fn sources(set: DyeSet) -> Vec<Profile> {
    match set.entry().source {
        DyeSource::Profile(json) => vec![profile(&json)],
        DyeSource::Mean(sets) => sets.into_iter().flat_map(sources).collect(),
    }
}

/// A profile of a file the catalogue took, which read when it was taken.
pub fn profile(json: &str) -> Profile {
    Profile::read(json.as_bytes()).expect("a profile reads once its file is taken")
}

/// The wavelengths a set's dyes and base are known on, each with the three dyes' densities and the base's.
pub fn measured(set: DyeSet) -> Vec<(usize, [f64; 4])> {
    match set.entry().source {
        DyeSource::Profile(json) => bake::scanned(&profile(&json)),
        DyeSource::Mean(sets) => mean(&sets.into_iter().map(measured).collect::<Vec<_>>()),
    }
}

/// The profiles' mean at each wavelength more than half of them measure.
fn mean(sets: &[Vec<(usize, [f64; 4])>]) -> Vec<(usize, [f64; 4])> {
    (0..cie::WAVELENGTHS)
        .filter_map(|l| {
            let here: Vec<[f64; 4]> = sets.iter().filter_map(|s| s.iter().find(|(at, _)| *at == l).map(|(_, v)| *v)).collect();
            (2 * here.len() > sets.len()).then(|| (l, std::array::from_fn(|k| here.iter().map(|v| v[k]).sum::<f64>() / here.len() as f64)))
        })
        .collect()
}

/// The camera's density of the film holding `amounts` of each dye, and its change with each, in doubles.
pub fn read(measured: &[(usize, [f64; 4])], weights: &[[f64; 3]], amounts: [f64; 3]) -> ([f64; 3], M3) {
    let mut light = [0.0; 3];
    let mut slope = [[0.0; 3]; 3];
    for (l, d) in measured {
        let t = 10f64.powf(-(amounts[0] * d[0] + amounts[1] * d[1] + amounts[2] * d[2] + d[3]));
        for c in 0..3 {
            let passed = t * weights[*l][c];
            light[c] += passed;
            for k in 0..3 {
                slope[c][k] += passed * d[k];
            }
        }
    }
    (light.map(|v| -v.log10()), std::array::from_fn(|c| slope[c].map(|v| v / light[c])))
}

fn derive(measured: Vec<(usize, [f64; 4])>) -> Dyes {
    let weights = bake::camera_weights(&cie::D50);
    let to_rgb = bake::scan_matrix(&cie::D50);
    let (base, _) = read(&measured, &weights, [0.0; 3]);
    let (_, slope) = read(&measured, &weights, [1.0; 3]);
    let narrow = |m: &M3| m.map(|r| r.map(|v| v as f32));
    Dyes {
        dyes: measured.iter().map(|(_, d)| d.map(|v| v as f32)).collect(),
        weights: measured.iter().map(|(l, _)| weights[*l].map(|v| v as f32)).collect(),
        bare: bake::apply(&to_rgb, base.map(|v| 10f64.powf(-v))).map(|v| v as f32),
        to_camera: narrow(&bake::inverse(&to_rgb)),
        base: base.map(|v| v as f32),
        matrix: narrow(&bake::inverse(&slope)),
    }
}

/// The set's dyes, derived from its profiles at first use and again once its file changes.
pub fn dyes(set: DyeSet) -> &'static Dyes {
    static SETS: Baked<(DyeSet, u64), Dyes> = Baked::new();
    let entry = set.entry();
    SETS.get((entry.set, entry.revision), || derive(measured(entry.set)))
}

impl Dyes {
    /// The bare film's density as the scan reads it, on Rec. 2020.
    pub fn bare_density(&self) -> [f32; 3] {
        self.bare.map(|v| -v.log10())
    }

    /// The kernel's arguments on a frame whose bare film reads `black` and that unmixes at `dose`, its dye
    /// amounts read as they are.
    pub fn separation(&'static self, black: [f32; 3], dose: f32) -> DyeSeparation {
        let tone = [[0.0; 3]; oni_render::DYE_TONE_NODES];
        let (dyes, weights) = (&self.dyes, &self.weights);
        let (bare, to_camera, base, matrix) = (self.bare, self.to_camera, self.base, self.matrix);
        DyeSeparation { black, bare, to_camera, base, matrix, dose, dyes, weights, tone, tone_low: [0.0; 3], tone_step: [1.0; 3] }.as_amounts()
    }
}
