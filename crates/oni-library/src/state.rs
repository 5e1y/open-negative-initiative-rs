//! The files of `<support>/State/`, where no build before them looks: those builds set aside a file
//! they cannot parse, so a changed shape never stands at a name they read. ONI is their one writer.

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

/// The folder of the support files whose shape this build owns.
pub fn folder(support: &Path) -> PathBuf {
    support.join("State")
}

#[derive(Debug, PartialEq)]
pub enum Read<T> {
    Absent,
    Current(T),
    /// A newer build's file, as far as this one reads it: shown, never written.
    Newer(i64, Option<T>),
    /// Set aside when it does not parse; kept in place when the disk refuses to read it.
    Unreadable,
}

/// A newer file is left exactly as it is; an unreadable one is set aside at once.
pub fn read<T: DeserializeOwned>(path: &Path, current: i64) -> Read<T> {
    #[derive(serde::Deserialize)]
    struct Stamp {
        version: i64,
    }
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Read::Absent,
        Err(_) => return Read::Unreadable,
    };
    if let Ok(Stamp { version }) = serde_json::from_slice(&bytes)
        && version > current
    {
        return Read::Newer(version, serde_json::from_slice(&bytes).ok());
    }
    match serde_json::from_slice(&bytes) {
        Ok(value) => Read::Current(value),
        Err(_) => {
            crate::set_aside(path);
            Read::Unreadable
        }
    }
}

/// Indented `serde_json`, a newline last, written whole over a file this build can read or over none.
pub fn write<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut bytes = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    crate::replace_readable(path, &bytes)
}

/// What a refused write says while a newer build's file stands: the notice the person reads.
pub fn left_newer() -> std::io::Error {
    std::io::Error::other(oni_i18n::tr("This was last written by a newer version of ONI and is left as it is."))
}
