//! The camera a RAW names, read from its metadata by LibRaw without decoding a pixel.

use crate::DecodeError;
use crate::source::{Route, Source};
use std::ffi::CStr;
use std::os::raw::{c_char, c_float, c_int, c_void};
use std::path::Path;

unsafe extern "C" {
    fn oni_identify(path: *const c_char, stream: *mut c_void, make: *mut c_char, model: *mut c_char, dng: *mut c_int) -> c_int;
    fn oni_describe(path: *const c_char, stream: *mut c_void, out: *mut Described) -> c_int;
}

#[repr(C)]
struct Described {
    make: [c_char; 64],
    model: [c_char; 64],
    lens: [c_char; 128],
    artist: [c_char; 64],
    iso: c_float,
}

/// What a RAW says of its making, as LibRaw reads it: maker and model as it names them, the lens
/// the EXIF names, the artist, the ISO; empty or zero where the file says nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Description {
    pub make: String,
    pub model: String,
    pub lens: String,
    pub artist: String,
    pub iso: f32,
}

/// `path`'s description from its metadata alone; an error for a file LibRaw cannot open.
pub fn describe(path: &Path) -> Result<Description, DecodeError> {
    let mut source = Source::open(path, Route::PLATFORM)?;
    let mut out = Described { make: [0; 64], model: [0; 64], lens: [0; 128], artist: [0; 64], iso: 0.0 };
    let rc = unsafe { oni_describe(source.name(), source.take(), &mut out) };
    if rc != 0 {
        return Err(DecodeError(rc));
    }
    // SAFETY: the shim copies one byte short of each buffer, which it zeroed first.
    let text = |b: &[c_char]| unsafe { CStr::from_ptr(b.as_ptr()) }.to_string_lossy().trim().to_string();
    Ok(Description { make: text(&out.make), model: text(&out.model), lens: text(&out.lens), artist: text(&out.artist), iso: out.iso })
}

/// Maker and model as LibRaw names them, so two files of one body always share a key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Camera {
    pub make: String,
    pub model: String,
    pub dng: bool,
}

impl Camera {
    /// "Nikon Z f": the model alone when it already starts with the maker's name.
    pub fn name(&self) -> String {
        if self.model.to_lowercase().starts_with(&self.make.to_lowercase()) || self.make.is_empty() {
            self.model.clone()
        } else {
            format!("{} {}", self.make, self.model)
        }
    }
}

/// The camera `path` was made by; an error for a file LibRaw cannot open.
pub fn identify(path: &Path) -> Result<Camera, DecodeError> {
    identify_by(path, Route::PLATFORM)
}

pub(crate) fn identify_by(path: &Path, route: Route) -> Result<Camera, DecodeError> {
    let mut source = Source::open(path, route)?;
    let (mut make, mut model, mut dng) = ([0 as c_char; 64], [0 as c_char; 64], 0 as c_int);
    let rc = unsafe { oni_identify(source.name(), source.take(), make.as_mut_ptr(), model.as_mut_ptr(), &mut dng) };
    if rc != 0 {
        return Err(DecodeError(rc));
    }
    // SAFETY: the shim terminates both buffers within their 64 bytes.
    let text = |b: &[c_char; 64]| unsafe { CStr::from_ptr(b.as_ptr()) }.to_string_lossy().trim().to_string();
    Ok(Camera { make: text(&make), model: text(&model), dng: dng != 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_model_naming_its_maker_is_not_prefixed_twice() {
        let z = Camera { make: "Nikon".into(), model: "Z f".into(), dng: false };
        assert_eq!(z.name(), "Nikon Z f");
        let lumix = Camera { make: "Panasonic".into(), model: "Panasonic DC-S1R".into(), dng: false };
        assert_eq!(lumix.name(), "Panasonic DC-S1R");
        // Twin: a file that is not a RAW names no camera.
        let dir = std::env::temp_dir().join(format!("oni-identify-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let text = dir.join("scan.NEF");
        std::fs::write(&text, b"not a RAW").unwrap();
        assert!(identify(&text).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
