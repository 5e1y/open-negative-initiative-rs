//! Writing a file ONI makes without ever landing on an original: what stands at a destination, the
//! ledger of ONI's own exports, and every write through a temporary file then a rename.

use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

/// What a destination holds before a write. Its test asks the volume, so `P1.jpg` finds `P1.JPG`
/// where names ignore case and nothing where they do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Free,
    /// A file ONI exported there, unchanged since, that no sidecar adopted as a photo.
    Export,
    /// Anything else: an original, a file of another app, a folder, or one that cannot be read.
    Original,
}

pub fn standing(destination: &Path, ledger: &Ledger) -> Standing {
    match std::fs::symlink_metadata(destination) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Standing::Free,
        Ok(_) if ledger.vouches(destination) && !oni_settings::sidecar::url_for(destination).exists() => Standing::Export,
        _ => Standing::Original,
    }
}

/// The files ONI exported, each with the length and modification date it was left with, so a file
/// another hand wrote or changed since is never taken for one.
#[derive(Debug, Clone, Default)]
pub struct Ledger {
    file: Option<PathBuf>,
    book: Book,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Entry {
    path: String,
    length: u64,
    seconds: u64,
    nanos: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Book {
    files: Vec<Entry>,
}

const LEDGER: &str = "Exports.json";

/// The exports a ledger remembers, the oldest forgotten first: a forgotten one is renamed around.
const REMEMBERED: usize = 10_000;

impl Ledger {
    pub fn in_support(support: &Path) -> Ledger {
        let file = support.join(LEDGER);
        let book = oni_library::read_json(&file).unwrap_or_default();
        Ledger { file: Some(file), book }
    }

    /// A ledger that vouches for nothing: every file found is an original.
    pub fn none() -> Ledger {
        Ledger::default()
    }

    fn entry(file: &Path) -> Option<Entry> {
        let meta = std::fs::symlink_metadata(file).ok().filter(std::fs::Metadata::is_file)?;
        let at = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        let path = canonical(file).to_string_lossy().into_owned();
        Some(Entry { path, length: meta.len(), seconds: at.as_secs(), nanos: at.subsec_nanos() })
    }

    pub fn vouches(&self, file: &Path) -> bool {
        Ledger::entry(file).is_some_and(|e| self.book.files.contains(&e))
    }

    /// Keeps `file` as it stands now; a ledger that cannot be written only costs a later Replace.
    pub fn note(&mut self, file: &Path) -> io::Result<()> {
        let (Some(at), Some(entry)) = (self.file.as_deref(), Ledger::entry(file)) else { return Ok(()) };
        self.book.files.retain(|e| e.path != entry.path);
        self.book.files.push(entry);
        let over = self.book.files.len().saturating_sub(REMEMBERED);
        self.book.files.drain(..over);
        std::fs::create_dir_all(at.parent().unwrap_or(Path::new(".")))?;
        oni_settings::atomic::write_atomically(at, &serde_json::to_vec_pretty(&self.book).map_err(io::Error::other)?)
    }
}

/// Why a write did not land: something not ONI's stands there, or the disk refused.
#[derive(Debug)]
pub enum Refused {
    Taken,
    Io(io::Error),
}

impl From<io::Error> for Refused {
    fn from(e: io::Error) -> Refused {
        Refused::Io(e)
    }
}

/// A new file: refused when anything stands at `destination`, even one appearing while it is written,
/// save on a macOS exFAT or SMB volume, whose rename cannot refuse (`rename_new`).
pub fn create(destination: &Path, bytes: &[u8]) -> Result<(), Refused> {
    create_with(destination, bytes, &oni_library::roll_folder::rename_new)
}

/// The bytes staged, then renamed where nothing stands in one step: a write cut anywhere leaves the
/// name free, its temporary taken by the next export into the folder.
fn create_with(destination: &Path, bytes: &[u8], rename: &dyn Fn(&Path, &Path) -> io::Result<()>) -> Result<(), Refused> {
    let staged = oni_settings::atomic::staged(destination, bytes)?;
    rename(&staged, destination).map_err(|e| {
        let _ = std::fs::remove_file(&staged);
        if e.kind() == io::ErrorKind::AlreadyExists { Refused::Taken } else { Refused::Io(e) }
    })
}

/// A file ONI made before, swapped whole: until the rename lands the earlier one stays as it was.
pub fn replace(destination: &Path, bytes: &[u8]) -> Result<(), Refused> {
    let staged = oni_settings::atomic::staged(destination, bytes)?;
    oni_settings::atomic::renamed(&staged, destination).map_err(|e| {
        let _ = std::fs::remove_file(&staged);
        Refused::Io(e)
    })
}

/// The folders whose leftovers this run has taken: a leftover is a write whose process died, so one
/// look a folder spares a batch a listing per frame.
static SWEPT: std::sync::Mutex<std::collections::BTreeSet<PathBuf>> = std::sync::Mutex::new(std::collections::BTreeSet::new());

/// An export: a new file, or an earlier export replaced when `replacing`; never anything else. The
/// leftovers of exports cut short in its folder go at its first export of the run.
pub fn put(destination: &Path, bytes: &[u8], ledger: &Ledger, replacing: bool) -> Result<(), Refused> {
    let folder = destination.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    if SWEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(folder.to_path_buf()) {
        oni_settings::atomic::reclaimed(folder);
    }
    match standing(destination, ledger) {
        Standing::Free => create(destination, bytes),
        Standing::Export if replacing => replace(destination, bytes),
        _ => Err(Refused::Taken),
    }
}

/// The path with symlinks resolved where it exists: a file not yet written resolves its folder.
pub(crate) fn canonical(path: &Path) -> PathBuf {
    if let Ok(found) = std::fs::canonicalize(path) {
        return found;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(file)) => {
            match std::fs::canonicalize(if parent.as_os_str().is_empty() { Path::new(".") } else { parent }) {
                Ok(folder) => folder.join(file),
                Err(_) => PathBuf::from(oni_colour::profile::standardized(path)),
            }
        }
        _ => PathBuf::from(oni_colour::profile::standardized(path)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;

    /// Set in a child of the check below: the destination of an export it quits before the rename.
    const CUTTING: &str = "ONI_EXPORT_CUTTING";

    fn listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn an_export_cut_short_takes_no_name_and_its_leftover_goes_at_the_next_export() {
        let test = "file::tests::an_export_cut_short_takes_no_name_and_its_leftover_goes_at_the_next_export";
        if let Ok(target) = std::env::var(CUTTING) {
            // The bytes staged, then gone without unwinding before the rename, as a force-quit.
            let _ = create_with(Path::new(&target), b"whole", &|_, _| std::process::exit(0));
            return;
        }
        let dir = std::env::temp_dir().join(format!("oni-export-cut-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p1 = dir.join("P1.tif");
        let child = std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact", test]).env(CUTTING, &p1).output().unwrap();
        assert!(child.status.success(), "{child:?}");
        assert_eq!(standing(&p1, &Ledger::none()), Standing::Free, "a cut export takes no name: {:?}", listing(&dir));
        assert!(listing(&dir).iter().all(|n| n.ends_with(".tmp")), "{:?}", listing(&dir));
        // Twin: a name held by an empty file before the rename, cut there, reads as an original.
        let held = dir.join("P0.tif");
        drop(OpenOptions::new().write(true).create_new(true).open(&held).unwrap());
        assert_eq!(standing(&held, &Ledger::none()), Standing::Original);
        std::fs::remove_file(&held).unwrap();
        // Another export into the folder, under another name, takes the leftover.
        put(&dir.join("Q.tif"), b"q", &Ledger::none(), false).unwrap();
        assert_eq!(listing(&dir), ["Q.tif"]);
        // The folder is looked at once a run, not at each frame: a leftover laid after its first export stays.
        let late = dir.join(".oni-0123456789abcdef-0.tmp");
        std::fs::write(&late, b"").unwrap();
        put(&dir.join("R.tif"), b"r", &Ledger::none(), false).unwrap();
        assert!(late.exists());
        // Twin: the same leftover in a folder not yet exported to this run goes at its first export.
        let fresh = dir.join("fresh");
        std::fs::create_dir_all(&fresh).unwrap();
        std::fs::rename(&late, fresh.join(".oni-0123456789abcdef-0.tmp")).unwrap();
        put(&fresh.join("S.tif"), b"s", &Ledger::none(), false).unwrap();
        assert_eq!(listing(&fresh), ["S.tif"]);
        std::fs::remove_dir_all(&fresh).unwrap();
        std::fs::remove_file(dir.join("R.tif")).unwrap();
        // Twin: a file standing at the name is never replaced, and its export leaves no temporary.
        std::fs::write(&p1, b"original").unwrap();
        assert!(matches!(create(&p1, b"new"), Err(Refused::Taken)));
        assert_eq!((std::fs::read(&p1).unwrap(), listing(&dir)), (b"original".to_vec(), vec!["P1.tif".to_string(), "Q.tif".to_string()]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
