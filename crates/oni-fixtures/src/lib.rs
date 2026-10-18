//! The folders the checks read and never write: the reference roll, the Lumix calibration files and
//! the Swift app's dumps. Each lookup prints one line and answers `None` when its folder is absent.

use std::path::PathBuf;

/// Roll #8, the reference roll of Nikon NEFs.
pub const RAW_DIR: (&str, &str) = ("ONI_RAW_DIR", "/mnt/user-data/uploads/Downloads/Roll #8");
/// The Lumix S1R calibration RW2s.
pub const LUMIX_DIR: (&str, &str) = ("ONI_LUMIX_DIR", "/mnt/user-data/uploads/Negs");
/// The Swift app's `--decode` and `--render` dumps.
pub const REFERENCE_DIR: (&str, &str) = ("ONI_REFERENCE", "/mnt/user-data/uploads/Downloads/oni-reference");
/// The Swift app's renders of `oni-pipeline/tests/adversarial/branches.json`, by `scripts/adversarial-dump.sh`.
pub const ADVERSARIAL_DIR: (&str, &str) = ("ONI_ADVERSARIAL", "/mnt/user-data/uploads/Documents/oni-adversarial");

/// The folder `variable` names, else `default`, whether or not it exists.
pub fn folder((variable, default): (&str, &str)) -> PathBuf {
    std::env::var_os(variable).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(default))
}

/// The folder when it exists; otherwise `check` prints that it is skipped and gets `None`.
pub fn present(which: (&str, &str), check: &str) -> Option<PathBuf> {
    let dir = folder(which);
    if dir.is_dir() {
        Some(dir)
    } else {
        println!("SKIPPED {check}: no {} (set {})", dir.display(), which.0);
        None
    }
}

/// The reference roll, or `None` after a skip line.
pub fn raw_dir(check: &str) -> Option<PathBuf> {
    present(RAW_DIR, check)
}

/// The Lumix calibration files, or `None` after a skip line.
pub fn lumix_dir(check: &str) -> Option<PathBuf> {
    present(LUMIX_DIR, check)
}

/// The Swift dumps, or `None` after a skip line.
pub fn reference_dir(check: &str) -> Option<PathBuf> {
    present(REFERENCE_DIR, check)
}

/// False, after a skip line, when `ONI_SKIP_HEAVY` is set: CI runners hold no frame-sized GPU work.
pub fn heavy(check: &str) -> bool {
    if std::env::var_os("ONI_SKIP_HEAVY").is_some() {
        println!("SKIPPED {check}: ONI_SKIP_HEAVY is set");
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_folder_skips_and_a_present_one_is_found() {
        let here = env!("CARGO_MANIFEST_DIR");
        assert_eq!(present(("ONI_FIXTURES_UNSET_VARIABLE", here), "probe"), Some(PathBuf::from(here)));
        // Twin: a folder that cannot exist is refused rather than handed on.
        assert_eq!(present(("ONI_FIXTURES_UNSET_VARIABLE", "/nonexistent/oni"), "probe"), None);
    }
}
