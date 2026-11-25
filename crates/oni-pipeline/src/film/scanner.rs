//! ONI's scanner, the one reader of a colour negative a digital photo is shot on: a D50 lamp through the bare film and
//! its dyes, a Nikon D850's measured sensitivities, then the decode a scanned negative's RAW takes; and a film's own
//! filtration through it, each layer's exposure offset that lays a scene's mid grey on one share of the three windows.

use super::projection::{self, Scan};
use super::{GREY, bake, cie, profile::Profile, saturation};
use oni_render::Projection;
use serde::Deserialize;
use std::sync::LazyLock;

/// The sensor's sensitivities and LibRaw's matrix for it, with where each comes from.
const SENSOR: &str = include_str!("../../resources/film/scanner_nikon_d850.json");

/// What the scanner is, as the interface names it.
pub const SENSOR_NAME: &str = "Nikon D850";
pub const LAMP_NAME: &str = "D50";

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
    sensitivity: Vec<[f64; 3]>,
    colour_matrix: [f64; 9],
}

/// The lamp's light each of the sensor's channels takes, band by band, and the decode's matrix on those channels, the
/// open lamp's green reading one.
struct Scanner {
    seen: Vec<[f64; 3]>,
    to_working: bake::M3,
}

static SCANNER: LazyLock<Scanner> = LazyLock::new(|| {
    let d = serde_json::from_str::<File>(SENSOR).expect("the shipped sensor reads").data;
    let on_grid = d.wavelengths.len() == cie::WAVELENGTHS && d.wavelengths.iter().enumerate().all(|(i, w)| *w == cie::FIRST_NM + cie::STEP_NM * i as f64);
    assert!(on_grid && d.sensitivity.len() == cie::WAVELENGTHS, "the shipped sensor lies on the 380 to 780 nm grid");
    let seen: Vec<[f64; 3]> = (0..cie::WAVELENGTHS).map(|i| d.sensitivity[i].map(|v| v * cie::D50[i])).collect();
    // LibRaw's table holds the matrix in ten-thousandths, XYZ to the camera's channels.
    let cam_xyz: bake::M3 = std::array::from_fn(|r| std::array::from_fn(|c| d.colour_matrix[3 * r + c] / 1e4));
    let decode = oni_decode::working_from_camera(&cam_xyz).expect("the shipped sensor's matrix has an inverse");
    let open: [f64; 3] = std::array::from_fn(|c| seen.iter().map(|s| s[c]).sum());
    let green = bake::apply(&decode, open)[1];
    Scanner { seen, to_working: decode.map(|r| r.map(|v| v / green)) }
});

/// A colour negative of `profile` scanned: each layer's amount, coded as `10^-amount`, to the scan's light in working
/// Rec. 2020, the bare film reading its own orange, which its reference's gains even.
pub fn scan(profile: &Profile) -> Scan {
    let (dyes, base) = projection::dyes(profile);
    let table = projection::tabled(&dyes, &base, &SCANNER.seen);
    let projection = Projection { to_working: SCANNER.to_working.map(|r| r.map(|v| v as f32)), flare: 0.0, bands: cie::WAVELENGTHS };
    Scan { projection, table }
}

/// The film's own filtration through the scanner, the exposure offset a layer in log exposure, green's none: a scene's
/// mid grey then holds one share of the three windows, the bare film to the sensor's saturation, its dyes uncoupled; or
/// why no offset within reach does.
pub fn filtration(profile: &Profile) -> Result<[f64; 3], String> {
    let le = &profile.log_exposure;
    let (amounts, _) = bake::curves_through(le, &profile.density_curves, &[[0.0; 3]; 3]);
    let (dyes, base) = projection::dyes(profile);
    let scanner = &*SCANNER;
    let passed = |held: [f64; 3]| -> [f64; 3] {
        let through = |i: usize| 10f64.powf(-(base[i] + (0..3).map(|k| held[k] * dyes[i][k]).sum::<f64>()));
        let light: [f64; 3] = std::array::from_fn(|c| (0..cie::WAVELENGTHS).map(|i| through(i) * scanner.seen[i][c]).sum());
        bake::apply(&scanner.to_working, light)
    };
    let bare = passed([0.0; 3]);
    // Each working channel's density above the bare film, the film's layers exposed at `log`.
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
            if at(under) > 0.0 || at(over) < 0.0 || !at(under).is_finite() || !at(over).is_finite() {
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
