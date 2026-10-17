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

pub fn stamp(path: &Path) -> Option<Stamp> {
    let size = std::fs::metadata(path).ok()?.len() as i64;
    let text = exif::capture_date(path);
    let digest = sample_digest(path, size);
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

const BLOCK: usize = 16 * 1024;
const FRACTIONS: [f64; 3] = [0.25, 0.5, 0.75];

/// FNV-1a over three blocks from the middle of the file; `"0"` when nothing can be read, which
/// keeps the fingerprint well-formed rather than absent.
fn sample_digest(path: &Path, size: i64) -> String {
    digest_at(path, size, &FRACTIONS)
}

fn digest_at(path: &Path, size: i64, fractions: &[f64]) -> String {
    let Ok(mut file) = File::open(path) else { return "0".into() };
    if size <= 0 {
        return "0".into();
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for fraction in fractions {
        let offset = (size as f64 - BLOCK as f64).min(size as f64 * fraction).max(0.0) as u64;
        if file.seek(SeekFrom::Start(offset)).is_err() {
            continue;
        }
        let mut data = Vec::with_capacity(BLOCK);
        if (&mut file).take(BLOCK as u64).read_to_end(&mut data).is_err() {
            continue;
        }
        for byte in data {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{hash:x}")
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
