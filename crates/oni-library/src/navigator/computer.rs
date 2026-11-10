//! The home folder and the volumes, Computer's rows, and which paths lie on a volume.

use crate::paths::{self, holds, same};
use std::path::Path;

use super::probe;

/// The home folder and the folder volumes are mounted in, so a check walks a disk of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Computer {
    pub home: String,
    pub volumes: String,
}

/// `volumes` on Windows, which mounts no disk in one folder: each drive's root is a volume.
pub const DRIVES: &str = "*";

/// Whether `path` names a drive's root, `C:\` or `C:/`.
pub fn is_drive_root(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() == 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// The person's home folder as the system names it: on Windows `USERPROFILE`, as a `HOME` that Git
/// Bash or MSYS2 sets reads `/c/Users/a`, which no Windows call takes.
pub fn home_folder() -> std::path::PathBuf {
    let (first, second) = if cfg!(windows) { ("USERPROFILE", "HOME") } else { ("HOME", "USERPROFILE") };
    std::env::var_os(first).filter(|h| !h.is_empty()).or_else(|| std::env::var_os(second)).unwrap_or_default().into()
}

impl Computer {
    /// This machine's: `NSHomeDirectory()` resolved, and `/Volumes`, or the drives on Windows.
    pub fn system() -> Computer {
        let home = home_folder();
        let volumes = if cfg!(windows) { DRIVES } else { "/Volumes" };
        Computer { home: paths::canonical(Path::new(&home)), volumes: volumes.into() }
    }

    /// No disk at all: what a check gets unless it hands one, so no test reads the person's folders.
    pub fn none() -> Computer {
        Computer { home: String::new(), volumes: String::new() }
    }

    /// The home folder, then every volume but the boot one, in the Finder's order. Reads the disk,
    /// each drive or mount on a thread of its own: one that does not answer is left out until it does.
    pub fn computer(&self) -> Vec<String> {
        let home = Some(self.home.clone()).filter(|h| !h.is_empty());
        let volumes = if self.volumes == DRIVES { probe::drives() } else { probe::mounted(&self.volumes) };
        home.into_iter().chain(volumes).collect()
    }

    pub fn is_volume_root(&self, path: &str) -> bool {
        if self.volumes == DRIVES {
            return is_drive_root(path);
        }
        same(&paths::parent(path), &self.volumes)
    }

    /// On a disk that may come and go: under `/Volumes` in any spelling of it, or on Windows a drive
    /// other than the home's; never with no volumes' folder known, which would hold every path.
    pub fn is_on_volume(&self, path: &str) -> bool {
        if self.volumes.is_empty() {
            return false;
        }
        if self.volumes == DRIVES {
            let drive = |p: &str| p.get(..2).map(str::to_ascii_uppercase);
            return path.as_bytes().get(1) == Some(&b':') && drive(path) != drive(&self.home);
        }
        holds(&self.volumes, path) && !same(&self.volumes, path)
    }
}
