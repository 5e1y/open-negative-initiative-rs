//! A file's identity: its fingerprint leaves the file name out, so a renamed or moved file keeps it.

use crate::exif;
use chrono::{Local, NaiveDateTime, TimeZone};
use oni_settings::Date;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// The two facts an index row needs, off one reading of the metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct Stamp {
    pub fingerprint: String,
    pub captured_at: Option<Date>,
}

/// Size, EXIF capture date and a digest of a sample of the contents: the sample separates
/// uncompressed scans whose size and date alone collide.
pub fn fingerprint(path: &Path) -> Option<String> {
    stamp(path).map(|s| s.fingerprint)
}

/// `None` for a file that cannot be read whole or that the cloud keeps away: a print made of what
/// was not read would be shared by every such file of one size.
pub fn stamp(path: &Path) -> Option<Stamp> {
    let metadata = std::fs::metadata(path).ok()?;
    if in_cloud(&metadata) {
        return None;
    }
    let size = metadata.len() as i64;
    let digest = sample_digest(path, size)?;
    let text = exif::capture_date(path);
    // A literal marker fills the date slot when there is no EXIF date, so it cannot collide with one.
    Some(Stamp {
        fingerprint: format!("{size}-{}-{digest}", text.as_deref().unwrap_or("no-date")),
        captured_at: text.as_deref().and_then(date),
    })
}

/// Whether a fingerprint was stamped off another size of its file, as a scan read while still
/// being written is; a fingerprint whose head carries no size never is.
pub fn is_stale(fingerprint: &str, size: i64) -> bool {
    let head: String = fingerprint.chars().take_while(|&c| c != '-').collect();
    head.parse::<i64>().is_ok_and(|stamped| stamped != size)
}

/// What a folder chosen for one gone shares with it, each row looked for at its own place below.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Recognised {
    /// The rows whose file stands at its place under the new folder with the same fingerprint.
    pub found: usize,
    pub total: usize,
    /// The rows whose place under the new folder holds another file: their fingerprint is not its.
    pub other: Vec<String>,
}

/// Each row under `old`, by path and fingerprint, looked for at its place under `new` and stamped
/// there. Reads the disk: one stamp per row.
pub fn recognised(rows: &[(String, String)], old: &str, new: &str) -> Recognised {
    let mut out = Recognised { total: rows.len(), ..Recognised::default() };
    for (path, print) in rows {
        let there = crate::paths::moved(path, old, new);
        match fingerprint(Path::new(&there)) {
            Some(found) if found == *print => out.found += 1,
            Some(_) => out.other.push(path.clone()),
            None => {}
        }
    }
    out
}

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

const BLOCK: usize = 16 * 1024;
const FRACTIONS: [f64; 3] = [0.25, 0.5, 0.75];

/// FNV-1a over three blocks from the middle of the file, `"0"` for an empty one; `None` when the
/// file or a block refuses to be read.
fn sample_digest(path: &Path, size: i64) -> Option<String> {
    digest_at(path, size, &FRACTIONS)
}

fn digest_at(path: &Path, size: i64, fractions: &[f64]) -> Option<String> {
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

/// EXIF's `2026:06:18 14:32:10`, colons in the date too, read in the local time zone.
pub fn date(text: &str) -> Option<Date> {
    let naive = NaiveDateTime::parse_from_str(text, "%Y:%m:%d %H:%M:%S").ok()?;
    let local = Local.from_local_datetime(&naive).earliest()?;
    Some(Date::from_unix_seconds(local.timestamp() as f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn two_files_of_one_size_part_on_their_sample_and_a_renamed_one_keeps_its_print() {
        let scratch = Scratch::new("relink");
        let count = 128 * 1024;
        let a: Vec<u8> = (0..count).map(|i| ((i * 7) % 251) as u8).collect();
        let b: Vec<u8> = (0..count).map(|i| ((i * 13) % 251) as u8).collect();
        let (pa, pb, pc) =
            (scratch.path().join("a.dng"), scratch.path().join("b.dng"), scratch.path().join("a-renamed.dng"));
        std::fs::write(&pa, &a).unwrap();
        std::fs::write(&pb, &b).unwrap();
        std::fs::write(&pc, &a).unwrap();
        let (fa, fb, fc) = (fingerprint(&pa).unwrap(), fingerprint(&pb).unwrap(), fingerprint(&pc).unwrap());
        assert_ne!(fa, fb);
        let head = |f: &str| f.split('-').next().unwrap().to_owned();
        assert_eq!(head(&fa), head(&fb), "the size alone does not separate them");
        assert_eq!(fa, fc);
        assert!(fa.contains("-no-date-"));
        // Twin: a sample taken elsewhere in the same file is another digest.
        assert_ne!(digest_at(&pa, count as i64, &[0.1, 0.5, 0.75]), sample_digest(&pa, count as i64));
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_file_gets_no_fingerprint() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("relink-unreadable");
        let (a, b) = (scratch.path().join("scan_01.tif"), scratch.path().join("scan_02.tif"));
        std::fs::write(&a, vec![1u8; 40_000]).unwrap();
        std::fs::write(&b, vec![2u8; 40_000]).unwrap();
        for p in [&a, &b] {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o000)).unwrap();
        }
        let closed = (stamp(&a), stamp(&b));
        for p in [&a, &b] {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        // Two scans of one size, neither readable: no print each, so neither lends the other its tile.
        assert_eq!(closed, (None, None));
        assert!(!in_cloud(&std::fs::metadata(&a).unwrap()), "a file on the disk is not in the cloud");
        // Twin: readable again, each gets a print of its own.
        let (fa, fb) = (fingerprint(&a).unwrap(), fingerprint(&b).unwrap());
        assert_ne!(fa, fb);
    }

    #[test]
    fn a_folder_moved_whole_is_recognised_by_its_prints_and_another_file_is_told() {
        let scratch = Scratch::new("relink-recognised");
        let (old, new) = (scratch.path().join("Roll 12"), scratch.path().join("Lisbon"));
        std::fs::create_dir_all(new.join("Sub")).unwrap();
        let a: Vec<u8> = (0..40_000).map(|i| (i % 251) as u8).collect();
        let b: Vec<u8> = (0..40_000).map(|i| (i % 13) as u8).collect();
        std::fs::write(new.join("A.tif"), &a).unwrap();
        std::fs::write(new.join("Sub/B.tif"), &b).unwrap();
        let (old, new) = (old.to_string_lossy().into_owned(), new.to_string_lossy().into_owned());
        let print = |data: &[u8]| {
            let at = scratch.path().join("print.tif");
            std::fs::write(&at, data).unwrap();
            fingerprint(&at).unwrap()
        };
        let row = |rel: &str, data: &[u8]| (crate::paths::join(&old, rel), print(data));
        let rows = vec![row("A.tif", &a), row("Sub/B.tif", &b), row("Gone.tif", &a)];
        assert_eq!(recognised(&rows, &old, &new), Recognised { found: 2, total: 3, other: Vec::new() });
        // Twin: the same name over another file is no match, and is told so its print is not lent.
        let swapped = vec![row("A.tif", &b)];
        assert_eq!(recognised(&swapped, &old, &new), Recognised { found: 0, total: 1, other: vec![swapped[0].0.clone()] });
    }

    #[test]
    fn an_empty_file_is_stamped_with_a_well_formed_print() {
        let scratch = Scratch::new("relink-empty");
        let path = scratch.path().join("DSC_0001.NEF");
        std::fs::write(&path, b"").unwrap();
        assert_eq!(stamp(&path), Some(Stamp { fingerprint: "0-no-date-0".into(), captured_at: None }));
        // Twin: a file that is not there has no stamp at all, never an invented one.
        assert_eq!(stamp(&scratch.path().join("gone.NEF")), None);
    }

    #[test]
    fn staleness_reads_the_size_at_the_head() {
        let stamp = "1-2026:10:08 10:00:00-9f";
        assert!(!is_stale(stamp, 1));
        assert!(is_stale(stamp, 2));
        assert!(!is_stale("legacy-2026:10:08-9f", 1));
    }

    #[test]
    fn the_exif_spelling_reads_in_local_time() {
        let read = date("2026:06:18 14:32:10").expect("parsable");
        let back = chrono::DateTime::from_timestamp(read.unix_seconds() as i64, 0).unwrap().with_timezone(&Local);
        assert_eq!(back.format("%Y:%m:%d %H:%M:%S").to_string(), "2026:06:18 14:32:10");
        // Twin: the ISO spelling is not the EXIF one.
        assert_eq!(date("2026-06-18 14:32:10"), None);
    }
}
