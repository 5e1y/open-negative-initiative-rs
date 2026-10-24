//! The original a sidecar was written for, by its print (the library's fingerprint) under a key Swift
//! skips: a namesake copied in later, another body's or a reset counter's, never adopts its settings.

use crate::codec::{self, Envelope};
use crate::modes::{ConversionMode, Medal};
use serde_json::Value;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// The key, Rust's own: Swift decodes past it, and a Swift rewrite drops it, which reads as unknown.
pub const KEY: &str = "originalPrint";

const BLOCK: usize = 16 * 1024;
const FRACTIONS: [f64; 3] = [0.25, 0.5, 0.75];

/// `SF_DATALESS`: iCloud keeps the file's bytes away until it is read.
#[cfg(target_os = "macos")]
const DATALESS: u32 = 0x4000_0000;
/// `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`, `RECALL_ON_OPEN` and `OFFLINE`: OneDrive's placeholders.
#[cfg(windows)]
const PLACEHOLDER: u32 = 0x0040_0000 | 0x0004_0000 | 0x0000_1000;

/// A file whose bytes live in the cloud: reading a block of it downloads it whole, so it is not
/// stamped until it is opened.
pub fn in_cloud(metadata: &std::fs::Metadata) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        if metadata.st_flags() & DATALESS != 0 {
            return true;
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & PLACEHOLDER != 0 {
            return true;
        }
    }
    let _ = metadata;
    false
}

/// FNV-1a over a block at each of `fractions` of the file, `"0"` for an empty one; `None` when the
/// file or a block refuses to be read.
pub fn digest_at(path: &Path, size: i64, fractions: &[f64]) -> Option<String> {
    let mut file = File::open(path).ok()?;
    if size <= 0 {
        return Some("0".into());
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for fraction in fractions {
        let offset = (size as f64 - BLOCK as f64).min(size as f64 * fraction).max(0.0) as u64;
        file.seek(SeekFrom::Start(offset)).ok()?;
        let mut data = Vec::with_capacity(BLOCK);
        (&mut file).take(BLOCK as u64).read_to_end(&mut data).ok()?;
        for byte in data {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    Some(format!("{hash:x}"))
}

/// A file's print, `size-date-digest`, as Swift's `Relink` spells it: `no-date` fills the date's
/// place, so it cannot collide with one. `None` for a file unread or kept away by the cloud.
pub fn print(path: &Path) -> Option<(String, Option<String>)> {
    let metadata = std::fs::metadata(path).ok()?;
    if in_cloud(&metadata) {
        return None;
    }
    let size = metadata.len() as i64;
    let digest = digest_at(path, size, &FRACTIONS)?;
    let date = crate::exif::capture_date(path);
    Some((format!("{size}-{}-{digest}", date.as_deref().unwrap_or("no-date")), date))
}

/// A print's date and digest; its size moves with any edit, so it is left out of the comparison.
fn parts(print: &str) -> Option<(&str, &str)> {
    let (_, rest) = print.split_once('-')?;
    let (date, digest) = rest.rsplit_once('-')?;
    Some((date, digest))
}

/// Whether `stored` was written for another file than one printed `now`: two dates and two digests,
/// as a date set right in place keeps its bytes, a file saved over its date; no date is never foreign.
pub fn foreign(stored: &Value, now: Option<&str>) -> bool {
    let (Some(was), Some(now)) = (stored.get(KEY).and_then(Value::as_str).and_then(parts), now.and_then(parts)) else { return false };
    let dated = |d: &str| d != "no-date";
    dated(was.0) && dated(now.0) && was.0 != now.0 && was.1 != now.1
}

/// `original`'s print, the one a write stores. Reads three blocks of the file.
pub fn of(original: &Path) -> Option<String> {
    print(original).map(|(text, _)| text)
}

/// The sidecar's text as stored, `None` when absent or not JSON.
pub(super) fn stored(path: &Path) -> io::Result<Option<Value>> {
    match std::fs::read(path) {
        Ok(data) => Ok(serde_json::from_slice(&data).ok()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// What a photo with no sidecar says: no medal, no roll, not in the ONI Trash.
pub fn blank() -> Envelope {
    Envelope {
        imported_at: None,
        label: Medal::None,
        exif: None,
        roll_draft: None,
        trashed_at: None,
        film: None,
        manual_order: None,
        mode: ConversionMode::default(),
        pass_of: None,
        leads_passes: false,
    }
}

/// `codec::envelope` for `original`'s sidecar `data`: a foreign one reads blank, as no sidecar does.
pub fn envelope(data: &[u8], original: &Path) -> Option<Envelope> {
    let value: Value = serde_json::from_slice(data).ok()?;
    if value.get(KEY).is_some() && foreign(&value, of(original).as_deref()) {
        return Some(blank());
    }
    codec::envelope(data)
}

/// A foreign sidecar moved aside, never lost: `X.NEF (orphan).json`, which no survey reads as a
/// sidecar since `X.NEF (orphan)` is no image's name; numbered when taken.
pub(super) fn set_aside(path: &Path) -> io::Result<PathBuf> {
    let folder = path.parent().unwrap_or(Path::new("."));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = name.strip_suffix(".json").unwrap_or(&name);
    for rank in 1..1000 {
        let tail = if rank == 1 { " (orphan).json".to_owned() } else { format!(" (orphan {rank}).json") };
        // A name past every file system's 255 keeps its tail and as much of its head as fits.
        let mut head: String = stem.to_owned();
        while head.chars().count() + tail.chars().count() > 255 {
            head.pop();
        }
        let target = folder.join(format!("{head}{tail}"));
        match std::fs::symlink_metadata(&target) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                std::fs::rename(path, &target)?;
                return Ok(target);
            }
            Err(e) => return Err(e),
            Ok(_) => {}
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "no free name for the orphan sidecar"))
}
