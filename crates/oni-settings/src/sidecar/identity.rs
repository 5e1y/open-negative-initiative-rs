//! The original a sidecar was written for, by its print (the library's fingerprint): a namesake copied
//! in later, another body's or a reset counter's, never adopts its settings.

use crate::document::{self, Envelope, Original};
use crate::modes::{ConversionMode, Medal};
use serde::Deserialize;
use serde_json::Value;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};


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
/// A last block alike (`tail`) is the same photo whose date a tool set right by writing it anew.
pub fn foreign(stored: &Original, now: Option<&str>, tail: impl FnOnce() -> Option<String>) -> bool {
    let (Some(was), Some(now)) = (parts(&stored.print), now.and_then(parts)) else { return false };
    let dated = |d: &str| d != "no-date";
    let apart = dated(was.0) && dated(now.0) && was.0 != now.0 && was.1 != now.1;
    apart && stored.tail.as_deref().is_none_or(|was| tail().as_deref() != Some(was))
}

/// Where a sidecar names its original: `original`, or beside the envelope in format 21 and under.
#[derive(Deserialize)]
struct Identity {
    #[serde(default)]
    original: Option<Value>,
    #[serde(rename = "originalPrint", default)]
    print: Option<Value>,
    #[serde(rename = "originalTail", default)]
    tail: Option<Value>,
}

/// The original a sidecar's `bytes` name, in either shape a version wrote it; `None` when they name none.
pub fn original_in(bytes: &[u8]) -> Option<Original> {
    let found: Identity = serde_json::from_slice(bytes).ok()?;
    let text = |v: Option<&Value>| v.and_then(Value::as_str).map(str::to_owned);
    match &found.original {
        Some(o) => Some(Original { print: text(o.get("print"))?, tail: text(o.get("tail")) }),
        None => Some(Original { print: text(found.print.as_ref())?, tail: text(found.tail.as_ref()) }),
    }
}

/// Whether the sidecar `bytes` were written for another file than `original`; its print is read only
/// when they name one.
pub fn foreign_bytes(bytes: &[u8], original: &Path) -> bool {
    original_in(bytes).is_some_and(|stored| foreign(&stored, of(original).as_deref(), || tail(original)))
}

/// `foreign_bytes` against a print and a tail already read.
pub fn foreign_with(bytes: &[u8], now: Option<&str>, tail: impl FnOnce() -> Option<String>) -> bool {
    original_in(bytes).is_some_and(|stored| foreign(&stored, now, tail))
}

/// `original`'s print, the one a write stores. Reads three blocks of the file.
pub fn of(original: &Path) -> Option<String> {
    print(original).map(|(text, _)| text)
}

/// Byte values a last block must hold to tell one photo: a scan's clipped or black bottom rows end
/// many files alike, and name no photo.
const TELLING: usize = 64;

/// The digest of `original`'s last block, which a write stores beside its print; none for a block
/// too uniform to tell a photo (`TELLING`). Reads one block.
pub fn tail(original: &Path) -> Option<String> {
    let metadata = std::fs::metadata(original).ok()?;
    if in_cloud(&metadata) {
        return None;
    }
    let mut file = File::open(original).ok()?;
    file.seek(SeekFrom::Start(metadata.len().saturating_sub(BLOCK as u64))).ok()?;
    let mut data = Vec::with_capacity(BLOCK);
    file.take(BLOCK as u64).read_to_end(&mut data).ok()?;
    let mut seen = [false; 256];
    data.iter().for_each(|b| seen[*b as usize] = true);
    if seen.iter().filter(|s| **s).count() < TELLING {
        return None;
    }
    digest_at(original, metadata.len() as i64, &[1.0])
}

/// What a photo with no sidecar says: no medal, no roll, not in the ONI Trash.
pub fn blank() -> Envelope {
    Envelope {
        imported_at: None,
        label: Medal::None,
        trashed_at: None,
        film: None,
        manual_order: None,
        mode: ConversionMode::default(),
        pass_of: None,
        leads_passes: false,
        sheet: false,
    }
}

/// `document::envelope` for `original`'s sidecar `data`: a foreign one reads blank, as no sidecar does.
pub fn envelope(data: &[u8], original: &Path) -> Option<Envelope> {
    if foreign_bytes(data, original) {
        return Some(blank());
    }
    document::envelope(data)
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
