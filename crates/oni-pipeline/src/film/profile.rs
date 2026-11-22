//! A spektrafilm film profile and the spectra table its exposure reads, as their files hold them.

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
    hanatos2025_adaptation_window_params: [f64; 4],
    #[serde(default)]
    layers_as_printed: bool,
}

/// What the bake reads of a profile; a wavelength the profile leaves unmeasured is `None`.
pub struct Profile {
    pub log_sensitivity: Vec<[f64; 3]>,
    pub channel_density: Vec<[Option<f64>; 3]>,
    pub base_density: Vec<Option<f64>>,
    pub log_exposure: Vec<f64>,
    pub density_curves: Vec<[f64; 3]>,
    /// The band-pass window: its ultraviolet edge's centre and width, then its infrared edge's, in nm.
    pub window: [f64; 4],
    /// Each layer's sensitivity at its data sheet's level: the bake scales it so a grey under the stock's light
    /// exposes it at log 0, the curves' common log H. A spektrafilm profile comes balanced so.
    pub layers_as_printed: bool,
}

impl Profile {
    pub fn read(json: &[u8]) -> Result<Profile, String> {
        let file: File = serde_json::from_slice(json).map_err(|e| e.to_string())?;
        let d = file.data;
        let on_grid = d.wavelengths.len() == WAVELENGTHS
            && d.wavelengths.iter().enumerate().all(|(i, w)| *w == super::cie::FIRST_NM + super::cie::STEP_NM * i as f64);
        let sized = [d.log_sensitivity.len(), d.channel_density.len(), d.base_density.len()].iter().all(|n| *n == WAVELENGTHS);
        if !on_grid || !sized || d.log_exposure.len() != d.density_curves.len() || d.log_exposure.len() < 2 {
            return Err("a profile off the 380 to 780 nm grid or with mismatched curves".into());
        }
        let numbers = d.log_sensitivity.iter().flatten().chain(d.density_curves.iter().flatten()).chain(&d.log_exposure);
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
            window: d.hanatos2025_adaptation_window_params,
            layers_as_printed: d.layers_as_printed,
        })
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
