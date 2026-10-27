//! Each camera's rendition matrix: LibRaw's Rec. 2020 to Apple's `CIRAWFilter`'s for that camera,
//! measured once on a Mac, so a RAW keeps the Swift app's colours. A camera absent decodes as LibRaw.

use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

pub type M3 = [[f32; 3]; 3];

/// The table the process decodes with, by maker and model as LibRaw names them.
static TABLE: RwLock<Option<Arc<HashMap<(String, String), M3>>>> = RwLock::new(None);

/// The table's print, FNV-1a over its entries in order: what a decode kept or read ahead was folded with.
static PRINT: AtomicU64 = AtomicU64::new(0);

/// Replaces the table every later decode folds; empty restores LibRaw's own colours.
pub fn install(entries: impl IntoIterator<Item = (String, String, M3)>) {
    let table: HashMap<(String, String), M3> = entries.into_iter().map(|(make, model, m)| ((make, model), m)).collect();
    let mut sorted: Vec<_> = table.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let text = format!("{sorted:?}");
    let print = if table.is_empty() { 0 } else { text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)) };
    if let Ok(mut t) = TABLE.write() {
        *t = (!table.is_empty()).then(|| Arc::new(table));
        PRINT.store(print, Ordering::SeqCst);
    }
}

/// The installed table's print, 0 for none: a decode depends on it as on its file.
pub fn print() -> u64 {
    PRINT.load(Ordering::SeqCst)
}

/// The matrix installed for a camera, if any.
pub fn matrix_for(make: &str, model: &str) -> Option<M3> {
    TABLE.read().ok()?.as_ref()?.get(&(make.to_owned(), model.to_owned())).copied()
}

/// The decode's conversion with the camera's matrix after it; a DNG reads its own profile in
/// either app and keeps its conversion, as does a camera with no matrix.
pub fn fold(rendition: Option<&M3>, dng: bool, conversion: M3) -> M3 {
    match rendition {
        Some(m) if !dng => std::array::from_fn(|r| std::array::from_fn(|c| (0..3).map(|k| m[r][k] * conversion[k][c]).sum())),
        _ => conversion,
    }
}

/// `fold` for the camera a LibRaw handle names.
pub(crate) fn for_decode(make: &[c_char; 64], model: &[c_char; 64], dng: bool, conversion: M3) -> M3 {
    // SAFETY: the shim copies at most 63 bytes into zeroed 64-byte buffers.
    let text = |b: &[c_char; 64]| unsafe { CStr::from_ptr(b.as_ptr()) }.to_string_lossy().trim().to_string();
    fold(matrix_for(&text(make), &text(model)).as_ref(), dng, conversion)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZF: M3 = [[1.1243, -0.1665, -0.0092], [0.0277, 0.9493, -0.0627], [0.0104, -0.0133, 0.8966]];
    const CONVERSION: M3 = [[1.9, -0.6, -0.3], [-0.2, 1.5, -0.3], [0.05, -0.4, 1.35]];

    #[test]
    fn the_camera_s_matrix_follows_the_conversion_and_nothing_else_moves() {
        let folded = fold(Some(&ZF), false, CONVERSION);
        let raw = [0.31f32, 0.52, 0.17];
        let apply = |m: &M3, v: [f32; 3]| -> [f32; 3] { std::array::from_fn(|r| (0..3).map(|c| m[r][c] * v[c]).sum()) };
        let expected = apply(&ZF, apply(&CONVERSION, raw));
        let got = apply(&folded, raw);
        assert!((0..3).all(|c| (got[c] - expected[c]).abs() < 1e-6), "{got:?} {expected:?}");
        // Twin: no matrix and a DNG keep the conversion as it is; the matrix before it lands elsewhere.
        assert_eq!(fold(None, false, CONVERSION), CONVERSION);
        assert_eq!(fold(Some(&ZF), true, CONVERSION), CONVERSION);
        let before = apply(&apply_rows(&CONVERSION, &ZF), raw);
        assert!((0..3).any(|c| (before[c] - expected[c]).abs() > 1e-3));
    }

    fn apply_rows(a: &M3, b: &M3) -> M3 {
        std::array::from_fn(|r| std::array::from_fn(|c| (0..3).map(|k| a[r][k] * b[k][c]).sum()))
    }
}
