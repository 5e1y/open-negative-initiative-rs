//! The camera a RAW names, read from its metadata by LibRaw without decoding a pixel.

use crate::DecodeError;
use crate::source::{Route, Source};
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::path::Path;

unsafe extern "C" {
    fn oni_identify(path: *const c_char, stream: *mut c_void, make: *mut c_char, model: *mut c_char, dng: *mut c_int) -> c_int;
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
