//! A paper's gaze on a colour negative: an enlarger's lamp through the bare film and its dyes exposes the paper's three
//! layers on their spectral sensitivities, each channel the density its layer sees above the bare film; and the film's
//! own filtration through it, each layer's exposure offset that lays a scene's mid grey on one share of the windows.

use super::projection::{self, Scan};
use super::{GREY, bake, cie, profile::Profile, saturation};
use oni_render::Projection;
use serde::Deserialize;

/// An enlarger's lamp, a tungsten-halogen bulb.
pub const ENLARGER_KELVIN: f64 = 3200.0;

/// The filtration's grain as a file holds it: a hundred-thousandth of a log exposure.
const HELD: f64 = 1e5;

/// The log exposure a layer's filtration is sought within, five stops either way, and the rounds the two layers take
/// in turn, each halved `HALVINGS` times: a layer's offset moves the others' reading a little, which a few rounds settle.
const REACH: f64 = 1.5;
const ROUNDS: usize = 12;
const HALVINGS: usize = 40;

#[derive(Deserialize)]
struct File {
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    wavelengths: Vec<f64>,
    log_sensitivity: Vec<[f64; 3]>,
}

/// A paper's three layers' spectral sensitivities on the profiles' grid, red-sensitive first, as its file holds them.
pub struct Sensitivity {
    pub log_sensitivity: Vec<[f64; 3]>,
}

impl Sensitivity {
    pub fn read(json: &[u8]) -> Result<Sensitivity, String> {
        let d = serde_json::from_slice::<File>(json).map_err(|e| e.to_string())?.data;
        let on_grid = d.wavelengths.len() == cie::WAVELENGTHS
            && d.wavelengths.iter().enumerate().all(|(i, w)| *w == cie::FIRST_NM + cie::STEP_NM * i as f64);
        if !on_grid || d.log_sensitivity.len() != cie::WAVELENGTHS || !d.log_sensitivity.iter().flatten().all(|v| v.is_finite()) {
            return Err("a paper's sensitivities off the 380 to 780 nm grid or holding a number that is not finite".into());
        }
        Ok(Sensitivity { log_sensitivity: d.log_sensitivity })
    }
}

/// The lamp's light each of the paper's layers takes, band by band, the brightest at 1.
fn seen(paper: &Sensitivity) -> Vec<[f64; 3]> {
    let raw: Vec<[f64; 3]> = (0..cie::WAVELENGTHS)
        .map(|i| paper.log_sensitivity[i].map(|v| 10f64.powf(v) * projection::planck(cie::FIRST_NM + cie::STEP_NM * i as f64, ENLARGER_KELVIN)))
        .collect();
    let top = raw.iter().flatten().fold(0.0, |a: f64, v| a.max(*v));
    raw.iter().map(|s| s.map(|v| v / top)).collect()
}

/// A colour negative of `profile` read by `paper`: each layer's amount, coded as `10^-amount`, to the light each of the
/// paper's layers takes over what the bare film passes it, `10^-density` above the bare film.
pub fn scan(profile: &Profile, paper: &Sensitivity) -> Scan {
    let (dyes, base) = projection::dyes(profile);
    let table = projection::tabled(&dyes, &base, &seen(paper));
    // The bare film's light as the kernel itself sums it, so its own reading of the bare film is one.
    let plain = Projection { to_working: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], flare: 0.0, bands: cie::WAVELENGTHS };
    let bare = oni_render::cpu::projection([1.0; 4], &plain, &table);
    let to_working = std::array::from_fn(|r| std::array::from_fn(|c| if r == c { 1.0 / bare[r] } else { 0.0 }));
    Scan { projection: Projection { to_working, ..plain }, table }
}

/// The film's own filtration through `paper`, the exposure offset a layer in log exposure, green's none: a scene's mid
/// grey then holds one share of the three windows, the bare film to the sensor's saturation, its dyes uncoupled; or why
/// no offset within reach does.
pub fn filtration(profile: &Profile, paper: &Sensitivity) -> Result<[f64; 3], String> {
    let le = &profile.log_exposure;
    let (amounts, _) = bake::curves_through(le, &profile.density_curves, &[[0.0; 3]; 3]);
    let (dyes, base) = projection::dyes(profile);
    let seen = seen(paper);
    let passed = |held: [f64; 3]| -> [f64; 3] {
        let through = |i: usize| 10f64.powf(-(base[i] + (0..3).map(|k| held[k] * dyes[i][k]).sum::<f64>()));
        std::array::from_fn(|c| (0..cie::WAVELENGTHS).map(|i| through(i) * seen[i][c]).sum())
    };
    let bare = passed([0.0; 3]);
    // Each of the paper's layers' density above the bare film, the film's layers exposed at `log`.
    let read = |log: [f64; 3]| -> [f64; 3] {
        let light = passed(std::array::from_fn(|c| bake::interpolate(log[c], le, &amounts, c)));
        std::array::from_fn(|c| -(light[c] / bare[c]).log10())
    };
    let white = (f64::from(saturation()) / f64::from(GREY)).log10();
    // How far layer `c`'s share of a mid grey lies over green's, the layers offset by `offset`.
    let over_green = |offset: [f64; 3], c: usize| -> f64 {
        let (at, top) = (read(offset), read(offset.map(|d| white + d)));
        at[c] / top[c] - at[1] / top[1]
    };
    let mut offset = [0.0f64; 3];
    for _ in 0..ROUNDS {
        for c in [0, 2] {
            let at = |d: f64| {
                let mut tried = offset;
                tried[c] = d;
                over_green(tried, c)
            };
            let (mut under, mut over) = (-REACH, REACH);
            if at(under) > 0.0 || at(over) < 0.0 {
                return Err("its layers hold no common share of a mid grey within five stops of filtration".into());
            }
            for _ in 0..HALVINGS {
                let mid = 0.5 * (under + over);
                if at(mid) < 0.0 { under = mid } else { over = mid }
            }
            offset[c] = 0.5 * (under + over);
        }
    }
    Ok(offset.map(|d| (d * HELD).round() / HELD))
}
