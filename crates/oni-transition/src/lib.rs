//! The transition from the Swift app: Apple's decode reads each camera a little differently from
//! LibRaw. Once per camera, on a Mac, the gap is fitted as a 3x3 on a few frames and kept in a table
//! every decode of that camera folds, so RAWs keep the earlier version's colours; no setting moves.

pub mod apple;
pub mod measure;
pub mod run;
pub mod survey;
pub mod table;

pub use apple::{AppleDecode, Simulated};
pub use run::{Event, Summary, pending, run};

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};

    /// An empty folder of the test's own under the system's temporary directory, removed on drop.
    pub struct Scratch(PathBuf);

    impl Scratch {
        pub fn new(label: &str) -> Scratch {
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
            let folder = std::env::temp_dir().join(format!("oni-transition-{label}-{}-{stamp}", std::process::id()));
            std::fs::create_dir_all(&folder).expect("scratch folder");
            Scratch(folder)
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
