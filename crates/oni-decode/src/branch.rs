//! The branch a file decodes through, sorted on its bytes, and what its name and depth say of it.

use crate::{container, plain};
use std::path::Path;

/// The branch a file decodes through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Branch {
    /// A camera mosaic, through LibRaw.
    Mosaic,
    /// A demosaiced linear scan in a TIFF container, through `container`.
    LinearRgb,
    /// Anything else, read by an image reader in its declared or chosen space.
    Plain,
}

/// Camera RAW extensions (`UTType.rawImage`), the fallback for a file whose bytes no probe claims.
const RAW_EXTENSIONS: &[&str] = &[
    "3fr", "ari", "arw", "bay", "braw", "cap", "cr2", "cr3", "crw", "dcr", "dcs", "dng", "drf", "eip", "erf", "fff", "gpr",
    "iiq", "k25", "kdc", "mdc", "mef", "mos", "mrw", "nef", "nrw", "orf", "pef", "ptx", "pxn", "r3d", "raf", "raw", "rw2",
    "rwl", "rwz", "sr2", "srf", "srw", "x3f",
];

/// Whether the extension names a camera RAW, consulted only when the bytes claim nothing.
pub fn is_raw(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| RAW_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Sorted on the bytes: a CFA is a mosaic, LinearRaw a linear scan; a file neither claims falls back
/// on the extension, so the probe only ever adds files to the linear branch.
pub fn route(path: &Path) -> Branch {
    match container::probe(path) {
        Some(container::Kind::Mosaic) => Branch::Mosaic,
        Some(container::Kind::LinearRgb) => Branch::LinearRgb,
        _ if is_raw(path) => Branch::Mosaic,
        _ => Branch::Plain,
    }
}

/// The extensions the image reader takes, its features in `Cargo.toml`: a file named otherwise
/// that no RAW engine reads is looked at for `unsupported_format`.
pub const PLAIN_EXTENSIONS: &[&str] = &["jpg", "jpeg", "jpe", "jfif", "png", "tif", "tiff", "gif", "bmp", "ico", "webp"];

/// Whether `path`'s name alone says a reader of ONI takes it; its bytes still decide (`route`).
pub fn named_decodable(path: &Path) -> bool {
    is_raw(path) || path.extension().and_then(|e| e.to_str()).is_some_and(|e| PLAIN_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// The format of a file neither LibRaw, the linear container nor an image reader takes: `HEIC`, `PSD`.
pub fn unsupported_format(path: &Path) -> Option<String> {
    (route(path) == Branch::Plain).then(|| plain::unsupported(path)).flatten()
}

/// Whether `path` decodes through LibRaw's mosaic, which the settings' migration shares with the decode.
pub fn decodes_via_mosaic(path: &Path) -> bool {
    route(path) == Branch::Mosaic
}

/// Whether a file holds 8 bits per channel or fewer: a warning and never a refusal, since
/// the density inversion bands low-code shadows. A RAW name is never low.
pub fn is_low_precision(path: &Path) -> bool {
    if container::probe(path) == Some(container::Kind::LinearRgb)
        && let Some(bits) = container::bits_per_sample(path)
    {
        return bits <= 8;
    }
    !is_raw(path) && image_depth(path).is_some_and(|bits| bits <= 8)
}

/// The depth an image reader reports: a TIFF's directory, a PNG's header, 8 for a JPEG.
fn image_depth(path: &Path) -> Option<usize> {
    use std::io::Read;
    let mut head = [0u8; 26];
    let n = std::fs::File::open(path).ok()?.read(&mut head).ok()?;
    match &head[..n.min(8)] {
        [0xFF, 0xD8, 0xFF, ..] => Some(8),
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] if n >= 25 => Some(head[24] as usize),
        [b'I', b'I', ..] | [b'M', b'M', ..] => container::bits_per_sample(path),
        _ => None,
    }
}
