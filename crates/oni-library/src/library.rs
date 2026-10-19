//! The library's members are every image under the roots, read off the disk; `library.json` is
//! read once, to tell a library made before the roots which photos no folder holds yet.

use crate::index::{Entry, LibraryIndex};
use crate::{paths, relink};
use oni_settings::codec::{self, Envelope};
use oni_settings::{ConversionMode, Date, Medal, sidecar};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryFile {
    pub version: i64,
    pub files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub folders: Option<Vec<String>>,
}

impl Default for LibraryFile {
    fn default() -> Self {
        LibraryFile { version: 2, files: Vec::new(), folders: None }
    }
}

/// The extensions macOS declares as images (`UTType.image` and its descendants, camera RAWs
/// included), lower case: the archive lists these and nothing else.
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "tif", "tiff", "gif", "bmp", "ico", "icns", "heic", "heics", "heif", "hif",
    "avif", "webp", "jp2", "j2k", "jpf", "jpx", "jpm", "psd", "tga", "exr", "hdr", "pbm", "pgm", "ppm", "pnm", "pict",
    "pct", "pic", "sgi", "svg", "dds", "ktx", "astc", "xbm", "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf",
    "sr2", "rw2", "raw", "rwl", "orf", "raf", "pef", "srw", "3fr", "fff", "iiq", "mos", "mrw", "x3f", "erf", "kdc",
    "dcr", "mef", "dcs", "gpr",
];

pub fn is_importable(path: &Path) -> bool {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.as_str()))
}

pub fn file_path(support: &Path) -> PathBuf {
    support.join("library.json")
}

pub fn load(support: &Path) -> LibraryFile {
    std::fs::read(file_path(support)).ok().and_then(|data| serde_json::from_slice(&data).ok()).unwrap_or_default()
}

pub use crate::paths::canonical;

/// What an older build wrote in place of a fingerprint it could not compute: those rows are
/// dropped rather than trusted.
pub const INVENTED_MARK: &str = "not-found-";
pub const WAITING_MARK: &str = "waiting-";

/// A tile laid while its file is still unread, ordered by the file's own date.
pub fn waiting(path: &str, dated: Option<Date>) -> Entry {
    let mut entry = Entry::new(path, dated.unwrap_or(crate::distant_past()), &format!("{WAITING_MARK}{path}"));
    entry.captured_at = dated;
    entry
}

/// An index row read from a member's own files, `None` when unreachable. Never invents: a made-up
/// fingerprint throws a valid thumbnail cache away, a made-up date piles photos onto one day.
pub fn indexed(path: &str) -> Option<Entry> {
    let original = Path::new(path);
    let stamp = relink::stamp(original)?;
    // The envelope alone, not the settings: a rebuild reads every member. No sidecar reads as a fresh one.
    let side = sidecar::url_for(original);
    let loaded: Option<Envelope> = if side.exists() {
        std::fs::read(&side).ok().and_then(|data| codec::envelope(&data))
    } else {
        Some(Envelope {
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
        })
    };
    Some(Entry {
        path: path.to_owned(),
        // A member with no readable sidecar still has its file, hence its own dates.
        imported_at: loaded.as_ref().and_then(|l| l.imported_at).unwrap_or_else(Date::now),
        captured_at: stamp.captured_at,
        fingerprint: stamp.fingerprint,
        label: loaded.as_ref().map(|l| l.label).unwrap_or_default(),
        // Membership lives in the sidecar and is only cached here, so the index can be thrown away.
        film: loaded.as_ref().and_then(|l| l.film.clone()),
        manual_order: loaded.as_ref().and_then(|l| l.manual_order),
        mode: loaded.as_ref().map(|l| l.mode),
        pass_of: loaded.as_ref().and_then(|l| l.pass_of.clone()),
        leads_passes: loaded.as_ref().is_some_and(|l| l.leads_passes),
        trashed_at: loaded.as_ref().and_then(|l| l.trashed_at),
    })
}

/// The members an index does not hold yet, everything a rebuild skipped because the disk was not
/// there: read on every reload, so plugging the disk back in is the whole repair.
pub fn unresolved(members: &[String], index: &LibraryIndex) -> Vec<String> {
    let held: HashSet<String> = index.all().into_iter().map(|e| e.path).collect();
    members.iter().filter(|m| !held.contains(*m)).cloned().collect()
}

/// Rows an older build invented, removed so a library carrying them heals on its next open.
pub fn drop_invented(index: &LibraryIndex) -> rusqlite::Result<usize> {
    let invented: Vec<String> =
        index.all().into_iter().filter(|e| e.fingerprint.starts_with(INVENTED_MARK)).map(|e| e.path).collect();
    for path in &invented {
        index.remove(path)?;
    }
    Ok(invented.len())
}

/// Moves a photograph's original out of the library into `folder`, its sidecar and row cleared:
/// the one case where the app moves an original, and only once a hand has confirmed it.
pub fn move_out(path: &str, folder: &Path, index: Option<&LibraryIndex>) -> Option<PathBuf> {
    std::fs::create_dir_all(folder).ok()?;
    let name = paths::name(path);
    let mut target = folder.join(&name);
    let mut rank = 2;
    while target.exists() {
        target = folder.join(format!("{} {rank}.{}", paths::stem(path), paths::extension(path)));
        rank += 1;
    }
    std::fs::rename(path, &target).ok()?;
    let _ = sidecar::remove(Path::new(path));
    if let Some(index) = index {
        let _ = index.remove(path);
    }
    Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn an_invented_row_is_dropped_and_a_waiting_one_kept() {
        let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
        let when = Date::since_reference_date(8e8);
        index.upsert(&Entry::new("/negs/a.RW2", crate::distant_past(), "not-found-/negs/a.RW2")).unwrap();
        index.upsert(&waiting("/negs/b.RW2", Some(when))).unwrap();
        index.upsert(&Entry::new("/negs/c.RW2", when, "100-no-date-9f")).unwrap();
        assert_eq!(drop_invented(&index).unwrap(), 1);
        let left: Vec<String> = index.all().into_iter().map(|e| e.path).collect();
        assert_eq!(left, ["/negs/b.RW2", "/negs/c.RW2"]);
        // Twin: a second pass over a healed index finds nothing to drop.
        assert_eq!(drop_invented(&index).unwrap(), 0);
        assert_eq!(waiting("/negs/b.RW2", None).imported_at, crate::distant_past());
    }

    #[test]
    fn moving_out_numbers_a_taken_name_and_clears_the_sidecar_and_the_row() {
        let scratch = Scratch::new("move-out");
        let original = scratch.path().join("x.RW2");
        std::fs::write(&original, b"frame").unwrap();
        std::fs::write(sidecar::url_for(&original), b"{}").unwrap();
        let away = scratch.path().join("Away");
        std::fs::create_dir(&away).unwrap();
        std::fs::write(away.join("x.RW2"), b"other").unwrap();
        let path = original.to_string_lossy().into_owned();
        let index = LibraryIndex::open(Path::new(":memory:")).unwrap();
        index.upsert(&Entry::new(&path, Date::since_reference_date(8e8), "5-no-date-1")).unwrap();

        let target = move_out(&path, &away, Some(&index)).unwrap();
        assert_eq!(target, away.join("x 2.RW2"));
        assert_eq!(std::fs::read(&target).unwrap(), b"frame");
        assert_eq!(std::fs::read(away.join("x.RW2")).unwrap(), b"other", "the taken name keeps its file");
        assert!(!original.exists() && !sidecar::url_for(&original).exists() && index.count() == 0);
        // Twin: a file that is not there moves nothing.
        assert_eq!(move_out(&path, &away, None), None);
    }
}
