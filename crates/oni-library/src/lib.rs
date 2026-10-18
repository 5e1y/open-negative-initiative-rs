//! The archive read off the disk: roots, folders and their rolls, projects, the ONI Trash, favourites,
//! and the SQLite index, a disposable cache rebuilt from the sidecars, which stay the only truth.

pub mod archive_tree;
pub mod capture;
pub mod exif;
pub mod favourites;
pub mod film_group;
pub mod folders;
pub mod gallery_history;
pub mod info;
pub mod index;
pub mod journal;
pub mod library;
pub mod navigator;
pub mod paths;
pub mod project;
pub mod project_tree;
pub mod relink;
pub mod rgb_passes;
pub mod roll_folder;
pub mod roll_membership;
pub mod roots;
pub mod trash;
pub mod wrapped;

pub use film_group::{FilmGroup, FilmKey};
pub use index::{Entry, LibraryIndex};
pub use oni_settings::{Date, Film, Id, Medal};

/// `Date.distantPast`, the date an undated value sorts before every real one with.
pub const DISTANT_PAST_SECONDS: f64 = -63_114_076_800.0;

pub fn distant_past() -> Date {
    Date::since_reference_date(DISTANT_PAST_SECONDS)
}

/// Seconds since 2001, the scale every order and comparison in the archive reads.
pub(crate) fn seconds(date: Date) -> f64 {
    date.time_interval_since_reference_date()
}

/// `date.addingTimeInterval(seconds)`.
pub fn later(date: Date, by: f64) -> Date {
    Date::since_reference_date(date.time_interval_since_reference_date() + by)
}

/// Writes beside the target, then renames: a reader never sees half a file.
pub(crate) fn write_atomically(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let directory = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let temporary = directory.join(format!(".{name}.{}.{stamp}.tmp", std::process::id()));
    let result = (|| {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// `Date` as a plain `JSONEncoder` writes a required one: seconds since 2001-01-01.
pub(crate) mod reference_date {
    use oni_settings::Date;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(date: &Date, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f64(date.time_interval_since_reference_date())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Date, D::Error> {
        Ok(Date::since_reference_date(f64::deserialize(d)?))
    }
}

#[cfg(test)]
pub(crate) mod scratch {
    use std::path::{Path, PathBuf};

    /// An empty folder of this test's own under the system's temporary directory, removed on drop.
    pub struct Scratch(pub PathBuf);

    impl Scratch {
        pub fn new(label: &str) -> Scratch {
            let stamp =
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
            let folder = std::env::temp_dir().join(format!("oni-library-{label}-{}-{stamp}", std::process::id()));
            std::fs::create_dir_all(&folder).expect("scratch folder");
            Scratch(PathBuf::from(crate::paths::canonical(&folder)))
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
