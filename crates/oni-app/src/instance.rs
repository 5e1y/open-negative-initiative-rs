//! One app at a time on a support folder: a lock the system lifts when the process ends, a crash
//! included, so a second launch never writes over what the first one holds in memory.

use oni_i18n::tr;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::Write as _;
use std::path::Path;

/// The lock's file in the support folder, which this app alone reads.
pub const FILE: &str = "instance.lock";

/// The lock, held while this value lives.
pub struct Instance(#[allow(dead_code)] File);

pub enum Held {
    Mine(Instance),
    /// Another process holds the folder.
    Elsewhere,
    /// The lock could not be taken at all: a support folder that refuses files writes nothing either.
    Unknown(std::io::Error),
}

pub fn hold(support: &Path) -> Held {
    let opened = std::fs::create_dir_all(support)
        .and_then(|()| OpenOptions::new().read(true).write(true).create(true).truncate(false).open(support.join(FILE)));
    let file = match opened {
        Ok(file) => file,
        Err(e) => return Held::Unknown(e),
    };
    match file.try_lock() {
        Ok(()) => Held::Mine(Instance(file)),
        Err(TryLockError::WouldBlock) => Held::Elsewhere,
        Err(TryLockError::Error(e)) => Held::Unknown(e),
    }
}

/// The second launch's words, on stderr and, for a launch by hand, in a window of the system's.
pub fn refused(support: &Path) {
    oni_i18n::set(crate::language::resolved(crate::language::chosen(&crate::preferences::Preferences::load(support))));
    let (title, message) = (tr("Open Negative Initiative is already open"), tr("Switch to its window: two copies at once would write over each other's work."));
    // A closed stderr fails the write, which `eprintln!` turns into a panic.
    let _ = writeln!(std::io::stderr().lock(), "{title}: {message} ({})", support.join(FILE).display());
    if by_hand() {
        crate::system::alert(title, message);
    }
}

/// A start that failed, said in a window of the system's to a launch by hand, the log named.
pub fn could_not_start(support: &Path, cause: &str) {
    oni_i18n::set(crate::language::resolved(crate::language::chosen(&crate::preferences::Preferences::load(support))));
    let log = crate::timing::log_path(support);
    let message = tr!("{cause}\n\nThe log holds the details: {log}", cause = cause, log = log.display().to_string());
    if by_hand() {
        crate::system::alert(&tr("Open Negative Initiative could not start"), &message);
    }
}

/// Neither a bench nor a launch behind the other windows, which a check makes and no one reads.
fn by_hand() -> bool {
    crate::bench::requested().is_none() && std::env::var_os("ONI_BACKGROUND").is_none_or(|v| v != "1")
}
