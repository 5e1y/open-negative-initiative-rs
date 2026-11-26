//! ONI's scanner, the one reader of a colour negative a digital photo is shot on: a D50 lamp through the bare film and
//! its dyes, a Nikon D850's measured sensitivities, then the decode a scanned negative's RAW takes.

use super::projection::{self, Scan};
use super::{bake, cie, profile::Profile};
use oni_render::Projection;
use serde::Deserialize;
use std::sync::LazyLock;

/// The sensor's sensitivities and LibRaw's matrix for it, with where each comes from.
const SENSOR: &str = include_str!("../../resources/film/scanner_nikon_d850.json");

/// What the scanner is, as the interface names it.
pub const SENSOR_NAME: &str = "Nikon D850";
pub const LAMP_NAME: &str = "D50";

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
