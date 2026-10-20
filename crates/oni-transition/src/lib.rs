//! The transition from the Swift app: RAWs it prepared were measured on Apple's decode, which reads
//! each camera a little differently from LibRaw. Once per camera the gap is measured on a few frames,
//! then every earlier sidecar of that camera has its gains shifted by it, after a backup.

pub mod apple;
pub mod correct;
pub mod measure;
pub mod records;
pub mod run;
pub mod survey;

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
