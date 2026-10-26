//! Writing a file ONI makes without ever landing on an original: what stands at a destination, the
//! ledger of ONI's own exports, and every write through a temporary file then a rename.

use std::fs::OpenOptions;
use std::io::{self, Write};
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
        oni_library::write_atomically(at, &serde_json::to_vec_pretty(&self.book).map_err(io::Error::other)?)
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

/// A new file: refused when anything stands at `destination`, even one appearing while it is written.
pub fn create(destination: &Path, bytes: &[u8]) -> Result<(), Refused> {
    let staged = staged(destination, bytes)?;
    // The name is held before the rename, so a file arriving meanwhile finds it taken.
    let held = OpenOptions::new().write(true).create_new(true).open(destination);
    if let Err(e) = held {
        let _ = std::fs::remove_file(&staged);
        return Err(if e.kind() == io::ErrorKind::AlreadyExists { Refused::Taken } else { Refused::Io(e) });
    }
    drop(held);
    oni_settings::atomic::renamed(&staged, destination).map_err(|e| {
        let _ = std::fs::remove_file(&staged);
        if std::fs::metadata(destination).is_ok_and(|m| m.len() == 0) {
            let _ = std::fs::remove_file(destination);
        }
        Refused::Io(e)
    })
}

/// A file ONI made before, swapped whole: until the rename lands the earlier one stays as it was.
pub fn replace(destination: &Path, bytes: &[u8]) -> Result<(), Refused> {
    let staged = staged(destination, bytes)?;
    oni_settings::atomic::renamed(&staged, destination).map_err(|e| {
        let _ = std::fs::remove_file(&staged);
        Refused::Io(e)
    })
}

/// An export: a new file, or an earlier export replaced when `replacing`; never anything else.
pub fn put(destination: &Path, bytes: &[u8], ledger: &Ledger, replacing: bool) -> Result<(), Refused> {
    match standing(destination, ledger) {
        Standing::Free => create(destination, bytes),
        Standing::Export if replacing => replace(destination, bytes),
        _ => Err(Refused::Taken),
    }
}

/// The bytes written and flushed beside `destination` under a hidden name no archive lists.
fn staged(destination: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let staged = oni_settings::atomic::temporary(destination);
    let written = (|| {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&staged)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    match written {
        Ok(()) => Ok(staged),
        Err(e) => {
            let _ = std::fs::remove_file(&staged);
            Err(e)
        }
    }
}

/// `name 2.ext`, `name 3.ext`…: the Finder's way of keeping both.
pub fn numbered(path: &Path, number: usize) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = match path.extension() {
        Some(e) => format!("{stem} {number}.{}", e.to_string_lossy()),
        None => format!("{stem} {number}"),
    };
    path.with_file_name(name)
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
