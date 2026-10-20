//! Which sidecars the earlier app wrote. No field or layout tells the two apps' sidecars apart, so
//! the rule is time: a RAW's sidecar last written before the transition was installed.

use crate::records::Ledger;
use oni_settings::sidecar;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Default, PartialEq)]
pub struct Found {
    /// RAWs whose sidecar the earlier app wrote and the ledger has not settled, sorted.
    pub raws: Vec<PathBuf>,
    /// Roots not reachable now, an unplugged disk: looked at again when they come back.
    pub unreachable: Vec<String>,
}

/// The sidecar's last write in Unix seconds, none when it is absent.
pub fn written_at(sidecar: &Path) -> Option<f64> {
    let modified = std::fs::metadata(sidecar).and_then(|m| m.modified()).ok()?;
    Some(modified.duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64()))
}

/// True for a RAW's sidecar written before `installed_at` and not yet settled by the ledger.
pub fn is_earlier(raw: &Path, installed_at: f64, ledger: &Ledger) -> bool {
    let path = sidecar::url_for(raw);
    oni_decode::is_raw(raw)
        && written_at(&path).is_some_and(|t| t < installed_at)
        && !ledger.settled(&path.to_string_lossy())
}

/// Every RAW under the reachable `roots` whose sidecar the earlier app wrote. Reads the disk.
pub fn survey(roots: &[String], installed_at: f64, ledger: &Ledger) -> Found {
    let (reachable, unreachable): (Vec<String>, Vec<String>) = roots.iter().cloned().partition(|r| Path::new(r).is_dir());
    let mut raws: Vec<PathBuf> = oni_library::roots::scan(&reachable)
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| is_earlier(p, installed_at, ledger))
        .collect();
    raws.sort();
    raws.dedup();
    Found { raws, unreachable }
}
