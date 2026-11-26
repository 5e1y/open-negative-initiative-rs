//! LibRaw's reading of a file without a demosaic: whether it opens, whether it is Nikon's HE, its sensor counts.

use crate::ffi::{Session, oni_mosaic, oni_open};
use crate::source::{Route, Source};
use crate::{DecodeError, NIKON_HE, NO_COLOUR_MATRIX};
use std::os::raw::c_int;
use std::path::Path;

/// Whether `path` is Nikon's High Efficiency RAW, told by LibRaw's identification alone.
pub fn is_nikon_he(path: &Path) -> bool {
    let Ok(mut source) = Source::open(path, Route::PLATFORM) else { return false };
    let mut s = Session(unsafe { std::mem::zeroed() });
    unsafe { oni_open(source.name(), source.take(), 1.0, 0, &mut s.0) == NIKON_HE }
}

/// Whether LibRaw opens and unpacks every pixel of `path`: a decode's own reading, no demosaic.
pub fn reads(path: &Path) -> bool {
    let Ok(mut source) = Source::open(path, Route::PLATFORM) else { return false };
    let mut s = Session(unsafe { std::mem::zeroed() });
    let opened = unsafe { oni_open(source.name(), source.take(), 1.0, 0, &mut s.0) } == 0;
    opened && s.unpack() == 0
}

/// A RAW's sensor counts as LibRaw unpacks them, before any demosaic or colour: the visible area, row by row.
pub struct Mosaic {
    pub width: usize,
    pub height: usize,
    pub counts: Vec<u16>,
    /// LibRaw's colour filter pattern, two bits a site over an 8 × 2 tile.
    pub filters: u32,
    pub black: u16,
    pub white: u16,
}

impl Mosaic {
    /// The colour, 0 red, 1 green, 2 blue, the site at (`x`, `y`) filters.
    pub fn colour(&self, x: usize, y: usize) -> usize {
        let c = (self.filters >> ((((y << 1) & 14) | (x & 1)) << 1)) & 3;
        if c == 3 { 1 } else { c as usize }
    }
}

/// `path`'s sensor counts, or why LibRaw holds no mosaic for it.
pub fn mosaic(path: &Path) -> Result<Mosaic, DecodeError> {
    let mut source = Source::open(path, Route::PLATFORM)?;
    let mut s = Session(unsafe { std::mem::zeroed() });
    let rc = unsafe { oni_open(source.name(), source.take(), 1.0, 0, &mut s.0) };
    if rc != 0 {
        return Err(DecodeError(rc));
    }
    let rc = s.unpack();
    if rc != 0 {
        return Err(DecodeError(rc));
    }
    let (mut counts, mut out) = (std::ptr::null(), [0 as c_int; 6]);
    if unsafe { oni_mosaic(&s.0, &mut counts, out.as_mut_ptr()) } != 0 || counts.is_null() {
        return Err(DecodeError(NO_COLOUR_MATRIX));
    }
    let [w, h, pitch, filters, black, white] = out.map(|v| v as usize);
    let rows = unsafe { std::slice::from_raw_parts(counts, pitch * (h - 1) + w) };
    let counts = (0..h).flat_map(|y| rows[y * pitch..y * pitch + w].iter().copied()).collect();
    Ok(Mosaic { width: w, height: h, counts, filters: filters as u32, black: black as u16, white: white as u16 })
}
