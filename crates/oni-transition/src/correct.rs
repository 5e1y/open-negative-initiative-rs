//! One sidecar corrected: its gains moved by the camera's shift, after a backup, through the
//! sidecar writer; and the backups put back.

use crate::records::{self, Entry, Ledger, Outcome};
use oni_settings::blocks::Gains;
use oni_settings::codec::{self, Source};
use oni_settings::{PipelineSettings, sidecar};
use std::path::Path;

/// The settings the Rust app renders as the Swift app did: every stored gain before the logarithm
/// moved by Apple's reading over LibRaw's. The densities, fractions at those gains, follow alone.
pub fn shifted(settings: &PipelineSettings, stops: [f64; 3]) -> PipelineSettings {
    let moved = |s: [f32; 3]| std::array::from_fn(|c| (s[c] + stops[c] as f32).clamp(Gains::RANGE.0, Gains::RANGE.1));
    let mut out = settings.clone();
    out.gains.stops = moved(out.gains.stops);
    if let Some(reference) = &mut out.roll_reference {
        reference.stops = moved(reference.stops);
    }
    out
}

fn entry(sidecar: &Path, outcome: Outcome, reason: &str, before: &str) -> Entry {
    Entry {
        sidecar: sidecar.to_string_lossy().into_owned(),
        outcome,
        reason: (!reason.is_empty()).then(|| reason.to_owned()),
        before: before.to_owned(),
        after: None,
        stops: None,
        backup: None,
        at: records::now(),
    }
}

/// Backs up and corrects `raw`'s sidecar, or says why it is left as it is; the line is not yet recorded.
pub fn correct(support: &Path, raw: &Path, stops: [f64; 3]) -> Entry {
    let path = sidecar::url_for(raw);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => return entry(&path, Outcome::Failed, &format!("unreadable: {e}"), ""),
    };
    let before = records::digest(&bytes);
    let source = Source { mosaic: &|| true, plain: &|| false };
    let decoded = match codec::decode_with(&bytes, &source, true) {
        Ok(d) => d,
        Err(e) => return entry(&path, Outcome::Skipped, &format!("unreadable settings: {e}"), &before),
    };
    if decoded.is_from_future() {
        return entry(&path, Outcome::Skipped, "written by a newer version", &before);
    }
    let s = &decoded.settings;
    if s.rgb_passes.is_some() || s.pass_of.is_some() {
        return entry(&path, Outcome::Skipped, "RGB passes", &before);
    }
    if !s.decoded_from_mosaic {
        return entry(&path, Outcome::Skipped, "not decoded as a RAW", &before);
    }
    let backup = match records::back_up(support, &path, &bytes) {
        Ok(name) => name,
        Err(e) => return entry(&path, Outcome::Failed, &format!("no backup: {e}"), &before),
    };
    // Another app may have written since the read; that write wins and the frame waits for the next run.
    if std::fs::read(&path).ok().as_deref() != Some(&bytes[..]) {
        return entry(&path, Outcome::Failed, "changed while correcting", &before);
    }
    if let Err(e) = sidecar::write(&shifted(s, stops), raw, sidecar::WriteOptions::default()) {
        return entry(&path, Outcome::Failed, &format!("not written: {e}"), &before);
    }
    let after = std::fs::read(&path).map(|b| records::digest(&b)).ok();
    Entry { after, stops: Some(stops), backup: Some(backup), ..entry(&path, Outcome::Corrected, "", &before) }
}

/// What putting the backups back did.
#[derive(Debug, Default, PartialEq)]
pub struct Restored {
    pub restored: Vec<String>,
    /// Sidecars written since their correction, kept as they are: `(sidecar, why)`.
    pub kept: Vec<(String, String)>,
}

/// Every corrected sidecar back to the bytes it was found with, unless it changed since.
pub fn restore(support: &Path) -> Restored {
    let ledger = Ledger::load(support);
    let mut out = Restored::default();
    let mut corrected: Vec<&Entry> = ledger.entries.values().filter(|e| e.outcome == Outcome::Corrected).collect();
    corrected.sort_by(|a, b| a.sidecar.cmp(&b.sidecar));
    for e in corrected {
        let now = std::fs::read(&e.sidecar).map(|b| records::digest(&b)).ok();
        if now.is_none() || now != e.after {
            out.kept.push((e.sidecar.clone(), "changed since its correction".into()));
            continue;
        }
        let kept = e.backup.as_ref().map(|name| records::backup_folder(support).join(name));
        match kept.and_then(|k| std::fs::read(k).ok()) {
            Some(bytes) if records::digest(&bytes) == e.before => match oni_library::write_atomically(Path::new(&e.sidecar), &bytes) {
                Ok(()) => out.restored.push(e.sidecar.clone()),
                Err(err) => out.kept.push((e.sidecar.clone(), err.to_string())),
            },
            _ => out.kept.push((e.sidecar.clone(), "its backup is missing or damaged".into())),
        }
    }
    out
}

#[cfg(test)]
#[path = "correct_tests.rs"]
mod tests;
