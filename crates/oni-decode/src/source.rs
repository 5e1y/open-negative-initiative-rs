//! How LibRaw reaches a file. macOS and Linux hand it the name, as they always have. Windows hands it
//! a handle Rust opened: a `char*` name passes through the ANSI code page and stops at 260
//! characters, and LibRaw's own open refuses every rename or move of the file while it reads.

use crate::DecodeError;
use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::path::Path;

/// LibRaw's `LIBRAW_IO_ERROR`, what its own open answers for a file it cannot read.
pub(crate) const IO_ERROR: i32 = -100009;

unsafe extern "C" {
    fn oni_stream(handle: isize) -> *mut c_void;
    fn oni_stream_free(stream: *mut c_void);
}

/// The two ways in; `PLATFORM` is the one this system decodes by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    Name,
    Handle,
}

impl Route {
    pub(crate) const PLATFORM: Route = if cfg!(windows) { Route::Handle } else { Route::Name };
}

/// A file ready for the shim: its name, and the stream over its handle on the handle route.
pub(crate) struct Source {
    name: CString,
    stream: *mut c_void,
}

impl Source {
    pub(crate) fn open(path: &Path, route: Route) -> Result<Source, DecodeError> {
        match route {
            Route::Name => {
                let name = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| DecodeError(-3))?;
                Ok(Source { name, stream: std::ptr::null_mut() })
            }
            Route::Handle => {
                // Rust's open shares the file for reading, writing and deleting, and takes long paths.
                let file = std::fs::File::open(path).map_err(|_| DecodeError(IO_ERROR))?;
                let stream = unsafe { oni_stream(raw(file)) };
                if stream.is_null() {
                    return Err(DecodeError(IO_ERROR));
                }
                Ok(Source { name: CString::default(), stream })
            }
        }
    }

    pub(crate) fn name(&self) -> *const c_char {
        self.name.as_ptr()
    }

    /// The stream, handed to the shim, which frees it from then on; null on the name route.
    pub(crate) fn take(&mut self) -> *mut c_void {
        std::mem::replace(&mut self.stream, std::ptr::null_mut())
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        if !self.stream.is_null() {
            unsafe { oni_stream_free(self.stream) };
        }
    }
}

#[cfg(windows)]
fn raw(file: std::fs::File) -> isize {
    use std::os::windows::io::IntoRawHandle;
    file.into_raw_handle() as isize
}

#[cfg(unix)]
fn raw(file: std::fs::File) -> isize {
    use std::os::unix::io::IntoRawFd;
    file.into_raw_fd() as isize
}

#[cfg(test)]
mod tests;
