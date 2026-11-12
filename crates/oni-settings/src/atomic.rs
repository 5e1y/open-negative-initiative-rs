//! A file written whole or not at all: a temporary beside the target, handed to the drive, then renamed.
//! The one writer of the app's files, sidecars, presets, preferences, index and exports alike.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// The bytes handed to the drive before the rename. A plain `fsync`: `sync_all`'s F_FULLFSYNC on macOS
/// costs nine times as much, 117 ms against 13 for a 38-frame roll's sidecars.
#[cfg(unix)]
fn flushed(file: &File) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn fsync(fd: i32) -> i32;
    }
    // SAFETY: the descriptor belongs to `file`, open for the whole call.
    if unsafe { fsync(file.as_raw_fd()) } == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

#[cfg(not(unix))]
fn flushed(file: &File) -> io::Result<()> {
    file.sync_all()
}

/// The temporaries of the writes in flight in this process, so two writes of one file never share one.
static IN_FLIGHT: Mutex<BTreeSet<PathBuf>> = Mutex::new(BTreeSet::new());

fn in_flight() -> std::sync::MutexGuard<'static, BTreeSet<PathBuf>> {
    IN_FLIGHT.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// `.oni-<hash of the target's name>-<rank>.tmp` beside `path`, about 30 bytes whatever the
/// target's: a name of 220 bytes or more plus its own would pass the 255 every file system allows.
fn temporary(path: &Path, rank: usize) -> PathBuf {
    let directory = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    directory.join(format!(".oni-{hash:016x}-{rank}.tmp"))
}

/// Whether `name` is a temporary's, whatever its target.
fn temporary_named(name: &str) -> bool {
    let Some(inner) = name.strip_prefix(".oni-").and_then(|n| n.strip_suffix(".tmp")) else { return false };
    let Some((hash, rank)) = inner.split_once('-') else { return false };
    hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()) && !rank.is_empty() && rank.bytes().all(|b| b.is_ascii_digit())
}

/// Every temporary in `dir` no write holds, in this process or another, as a write cut short leaves
/// one: an export reclaims its folder's, whose next write may never come under the same name.
pub fn reclaimed(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if !temporary_named(&entry.file_name().to_string_lossy()) {
            continue;
        }
        // A write holds its temporary's lock until its rename; the system lets it go when the writer dies.
        if let Ok(file) = File::open(entry.path())
            && file.try_lock().is_ok()
        {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// A temporary held for its write until dropped, named from its target and the lowest rank no
/// write of that target in flight holds: an interrupted write's leftover is taken by the next one.
/// Its file stays open and locked, so no reclaim takes it while it waits for its rename.
pub struct Staged {
    path: PathBuf,
    _locked: Option<File>,
}

impl Staged {
    fn held(path: &Path) -> Staged {
        let mut flying = in_flight();
        let free = (0..).map(|rank| temporary(path, rank)).find(|t| !flying.contains(t)).unwrap_or_default();
        flying.insert(free.clone());
        Staged { path: free, _locked: None }
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        in_flight().remove(&self.path);
    }
}

impl std::ops::Deref for Staged {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for Staged {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

/// macOS's `O_EXLOCK`: the file created and locked in one step, so no reclaim finds it unlocked.
#[cfg(target_os = "macos")]
const CREATED_LOCKED: i32 = 0x20;

/// The new temporary's lock, taken at its creation on macOS, just after it elsewhere.
fn locked(file: File) -> io::Result<File> {
    if cfg!(target_os = "macos") {
        return Ok(file);
    }
    file.try_lock().map_err(|e| match e {
        std::fs::TryLockError::Error(e) => e,
        std::fs::TryLockError::WouldBlock => io::Error::from(io::ErrorKind::WouldBlock),
    })?;
    Ok(file)
}

/// The pauses between tries, about half a second in all: an indexer, a cloud client or an
/// antivirus lets a file go within that, and a hold any longer is said instead.
pub const PAUSES: [Duration; 4] = [Duration::from_millis(30), Duration::from_millis(60), Duration::from_millis(120), Duration::from_millis(240)];

/// Whether a refused rename may pass once another program lets go: busy anywhere; on Windows a
/// sharing or lock violation (32, 33) or an access denied (5), as a file held open leaves it.
pub fn held(error: &io::Error, windows: bool) -> bool {
    error.kind() == io::ErrorKind::ResourceBusy || (windows && matches!(error.raw_os_error(), Some(5 | 32 | 33)))
}

/// `rename` tried again after each pause while the file is held; any other refusal, or the last
/// one, is returned at once.
pub fn retried(from: &Path, to: &Path, rename: &dyn Fn(&Path, &Path) -> io::Result<()>, pause: &dyn Fn(Duration), windows: bool) -> io::Result<()> {
    let mut pauses = PAUSES.iter();
    loop {
        match rename(from, to) {
            Err(e) if held(&e, windows) => match pauses.next() {
                Some(wait) => pause(*wait),
                None => return Err(e),
            },
            done => return done,
        }
    }
}

/// `std::fs::rename` through `retried`, on this system's rules.
pub fn renamed(from: &Path, to: &Path) -> io::Result<()> {
    retried(from, to, &|f, t| std::fs::rename(f, t), &std::thread::sleep, cfg!(windows))
}

/// `write` into a temporary beside `path`, owner-only on Unix when `private`, handed to the drive
/// when `flush`; the temporary removed on any failure.
fn staged_with(path: &Path, private: bool, flush: bool, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<Staged> {
    let mut temporary = Staged::held(path);
    // A leftover goes first, so neither its mode nor a link at its name reaches the new file. Another
    // ONI process writing the same file at the same instant can take the same name: a known limit.
    let _ = std::fs::remove_file(&temporary);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    #[cfg(target_os = "macos")]
    std::os::unix::fs::OpenOptionsExt::custom_flags(&mut options, CREATED_LOCKED);
    let result = options.open(&temporary).and_then(locked).and_then(|mut file| {
        write(&mut file)?;
        if flush { flushed(&file) } else { Ok(()) }.map(|()| file)
    });
    match result {
        Ok(file) => {
            temporary._locked = Some(file);
            Ok(temporary)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&temporary);
            Err(e)
        }
    }
}

/// A staged temporary renamed over `path`, or removed when the rename is refused.
fn landed(path: &Path, staged: io::Result<Staged>, rename: &dyn Fn(&Path, &Path) -> io::Result<()>) -> io::Result<()> {
    let temporary = staged?;
    rename(&temporary, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temporary);
    })
}

/// `path` written whole by `write`, handed to the drive, then renamed: a reader never sees half a file.
pub fn write_with(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    landed(path, staged_with(path, false, true, write), &renamed)
}

/// `write_with` for bytes in hand.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    written(path, bytes, &renamed)
}

/// `write_atomically` through its own rename, which a check makes fail.
pub fn written(path: &Path, bytes: &[u8], rename: &dyn Fn(&Path, &Path) -> io::Result<()>) -> io::Result<()> {
    landed(path, staged_with(path, false, true, |file| file.write_all(bytes)), rename)
}

/// `write_atomically` readable by its owner alone on Unix; on Windows the support folder sits in
/// the user's profile, whose ACL already keeps others out.
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    landed(path, staged_with(path, true, true, |file| file.write_all(bytes)), &renamed)
}

/// The bytes written and handed to the drive beside `path` under a hidden name no archive lists,
/// for a caller that lands them by its own rule while it holds them.
pub fn staged(path: &Path, bytes: &[u8]) -> io::Result<Staged> {
    staged_with(path, false, true, |file| file.write_all(bytes))
}

/// `write_with` without handing the bytes to the drive, for a file made again when lost.
pub fn cached_with(path: &Path, write: impl FnOnce(&mut File) -> io::Result<()>) -> io::Result<()> {
    landed(path, staged_with(path, false, false, write), &renamed)
}

#[cfg(test)]
mod tests;
