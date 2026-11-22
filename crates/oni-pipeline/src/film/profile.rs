//! A film profile written from its maker's data sheet and the spectra table its exposure reads, as their files hold them.

use super::cie::WAVELENGTHS;
use serde::Deserialize;

#[derive(Deserialize)]
struct File {
    data: Data,
}

#[derive(Deserialize)]
struct Data {
    wavelengths: Vec<f64>,
    log_sensitivity: Vec<[f64; 3]>,
    channel_density: Vec<[Option<f64>; 3]>,
    base_density: Vec<Option<f64>>,
    log_exposure: Vec<f64>,
    density_curves: Vec<[f64; 3]>,
    sheet_density_curves: Vec<[f64; 3]>,
}

/// What the bake reads of a profile; a wavelength the profile leaves unmeasured is `None`. Each layer's sensitivity
/// stands at its sheet's level: the bake scales it so a grey under the stock's light exposes it at log 0.
pub struct Profile {
    pub log_sensitivity: Vec<[f64; 3]>,
    pub channel_density: Vec<[Option<f64>; 3]>,
    pub base_density: Vec<Option<f64>>,
    pub log_exposure: Vec<f64>,
    /// The dyes' amounts each layer develops along `log_exposure`, in units of its own Status M density.
    pub density_curves: Vec<[f64; 3]>,
    /// The sheet's Status M curves above D-min: the densities a neutral reads, each dye's unwanted absorptions in them.
    pub sheet_density_curves: Vec<[f64; 3]>,
}

impl Profile {
    pub fn read(json: &[u8]) -> Result<Profile, String> {
        let file: File = serde_json::from_slice(json).map_err(|e| e.to_string())?;
        let d = file.data;
        let on_grid = d.wavelengths.len() == WAVELENGTHS
            && d.wavelengths.iter().enumerate().all(|(i, w)| *w == super::cie::FIRST_NM + super::cie::STEP_NM * i as f64);
        let sized = [d.log_sensitivity.len(), d.channel_density.len(), d.base_density.len()].iter().all(|n| *n == WAVELENGTHS);
        let curves = d.log_exposure.len() == d.density_curves.len() && d.log_exposure.len() == d.sheet_density_curves.len();
        if !on_grid || !sized || !curves || d.log_exposure.len() < 2 {
            return Err("a profile off the 380 to 780 nm grid or with mismatched curves".into());
        }
        let curves = d.density_curves.iter().chain(&d.sheet_density_curves).flatten();
        let numbers = d.log_sensitivity.iter().flatten().chain(curves).chain(&d.log_exposure);
        let known = d.channel_density.iter().flatten().chain(&d.base_density).flatten();
        if !numbers.chain(known).all(|v| v.is_finite()) {
            return Err("a profile holding a number that is not finite".into());
        }
        Ok(Profile {
            log_sensitivity: d.log_sensitivity,
            channel_density: d.channel_density,
            base_density: d.base_density,
            log_exposure: d.log_exposure,
            density_curves: d.density_curves,
            sheet_density_curves: d.sheet_density_curves,
        })
    }
}

#[derive(Deserialize)]
struct SilverFile {
    data: SilverData,
}

#[derive(Deserialize)]
struct SilverData {
    wavelengths: Vec<f64>,
    log_sensitivity: Vec<f64>,
    log_exposure: Vec<f64>,
    density_curve: Vec<f64>,
}

/// What the bake reads of a black and white profile: its one layer's sensitivity at its sheet's level, and its
/// development's curve above base and fog on log H less a grey's. The other developments its file keeps are not read.
pub struct Silver {
    pub log_sensitivity: Vec<f64>,
    pub log_exposure: Vec<f64>,
    pub density_curve: Vec<f64>,
}

impl Silver {
    pub fn read(json: &[u8]) -> Result<Silver, String> {
        let d = serde_json::from_slice::<SilverFile>(json).map_err(|e| e.to_string())?.data;
        let on_grid = d.wavelengths.len() == WAVELENGTHS
            && d.wavelengths.iter().enumerate().all(|(i, w)| *w == super::cie::FIRST_NM + super::cie::STEP_NM * i as f64);
        if !on_grid || d.log_sensitivity.len() != WAVELENGTHS || d.log_exposure.len() != d.density_curve.len() || d.log_exposure.len() < 2 {
            return Err("a black and white profile off the 380 to 780 nm grid or with mismatched curves".into());
        }
        if !d.log_sensitivity.iter().chain(&d.log_exposure).chain(&d.density_curve).all(|v| v.is_finite()) {
            return Err("a profile holding a number that is not finite".into());
        }
        Ok(Silver { log_sensitivity: d.log_sensitivity, log_exposure: d.log_exposure, density_curve: d.density_curve })
    }
}

/// An IEEE half to a double.
fn half(bits: u16) -> f64 {
    let sign = if bits >> 15 == 1 { -1.0 } else { 1.0 };
    let exponent = i32::from((bits >> 10) & 0x1f);
    let mantissa = f64::from(bits & 0x3ff);
    sign * match exponent {
        0 => mantissa * 2f64.powi(-24),
        31 if mantissa == 0.0 => f64::INFINITY,
        31 => f64::NAN,
        e => (1.0 + mantissa / 1024.0) * 2f64.powi(e - 15),
    }
}

/// The spectra table, `nodes` square, a spectrum of `WAVELENGTHS` samples per chromaticity node.
pub struct Spectra {
    pub nodes: usize,
    pub values: Vec<f64>,
}

impl Spectra {
    /// The `.npy` file as numpy writes a C-ordered `<f2` array of shape (n, n, 81).
    pub fn read(npy: &[u8]) -> Result<Spectra, String> {
        let bad = || "not a C-ordered half-float spectra table".to_string();
        if npy.get(..6) != Some(b"\x93NUMPY".as_slice()) || npy.len() < 10 {
            return Err(bad());
        }
        let header_len = usize::from(u16::from_le_bytes([npy[8], npy[9]]));
        let header = std::str::from_utf8(npy.get(10..10 + header_len).ok_or_else(bad)?).map_err(|_| bad())?;
        if !header.contains("'descr': '<f2'") || !header.contains("'fortran_order': False") {
            return Err(bad());
        }
        let shape = header.split("'shape': (").nth(1).and_then(|s| s.split(')').next()).ok_or_else(bad)?;
        let dims: Vec<usize> = shape.split(',').filter_map(|d| d.trim().parse().ok()).collect();
        let [n, m, w] = dims[..] else { return Err(bad()) };
        let body = &npy[10 + header_len..];
        if n != m || w != WAVELENGTHS || body.len() != n * m * w * 2 {
            return Err(bad());
        }
        let values = body.chunks_exact(2).map(|b| half(u16::from_le_bytes([b[0], b[1]]))).collect();
        Ok(Spectra { nodes: n, values })
    }

    pub fn at(&self, i: usize, j: usize) -> &[f64] {
        let start = (i * self.nodes + j) * WAVELENGTHS;
        &self.values[start..start + WAVELENGTHS]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_decode_as_ieee_writes_them() {
        assert_eq!(half(0x3c00), 1.0);
        assert_eq!(half(0xc000), -2.0);
        assert_eq!(half(0x0001), 2f64.powi(-24));
        assert_eq!(half(0x7bff), 65504.0);
        // Twin: a mantissa read off by one bit is another number.
        assert_ne!(half(0x3c01), 1.0);
    }
}
