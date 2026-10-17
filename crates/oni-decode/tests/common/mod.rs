//! The RAWs the checks read: `ONI_RAW_DIR`, else roll #8 as staged in the cloud workspace.
#![allow(dead_code)]

use std::path::PathBuf;

pub fn raw_dir() -> PathBuf {
    std::env::var_os("ONI_RAW_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/mnt/user-data/uploads/Downloads/Roll #8".into())
}

/// The folder's RAWs sorted by name, or None after printing why the check is skipped.
pub fn frames(check: &str, at_least: usize) -> Option<Vec<PathBuf>> {
    let dir = raw_dir();
    let mut out: Vec<_> = std::fs::read_dir(&dir)
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    out.retain(|p| {
        matches!(
            p.extension().and_then(|e| e.to_str()),
            Some("NEF" | "RW2" | "CR3" | "ARW" | "DNG")
        )
    });
    out.sort();
    if out.len() < at_least {
        println!(
            "skipped {check}: fewer than {at_least} RAW in {}; set ONI_RAW_DIR",
            dir.display()
        );
        return None;
    }
    Some(out)
}

pub fn means(px: &[[f32; 4]]) -> [f64; 3] {
    let mut s = [0f64; 3];
    for p in px {
        for c in 0..3 {
            s[c] += p[c] as f64;
        }
    }
    s.map(|v| v / px.len() as f64)
}
