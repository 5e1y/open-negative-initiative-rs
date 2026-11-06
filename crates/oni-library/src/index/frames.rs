//! The rows standing as frames: a pass its lead composes stands under its lead, and a contact sheet,
//! ONI's own file, under none.

use super::Entry;
use std::collections::HashSet;

/// What the gallery draws: every row but the ONI Trash's, the contact sheets and the passes whose
/// lead the library holds; a pass whose lead is gone shows again, so no scan vanishes for want of it.
pub fn shown(rows: &[Entry]) -> Vec<Entry> {
    let picked: Vec<&Entry> = rows.iter().filter(|e| e.trashed_at.is_none() && !e.sheet).collect();
    frames(&picked, rows)
}

/// The ONI Trash's rows, a triplet as its lead, latest deletion first.
pub fn trashed(rows: &[Entry]) -> Vec<Entry> {
    let picked: Vec<&Entry> = rows.iter().filter(|e| e.trashed_at.is_some()).collect();
    latest_first(frames(&picked, rows))
}

pub(super) fn latest_first(mut out: Vec<Entry>) -> Vec<Entry> {
    let at = |e: &Entry| e.trashed_at.map(crate::seconds).unwrap_or(crate::DISTANT_PAST_SECONDS);
    out.sort_by(|a, b| at(b).total_cmp(&at(a)));
    out
}

fn frames(picked: &[&Entry], rows: &[Entry]) -> Vec<Entry> {
    let held: HashSet<&str> = rows.iter().map(|e| e.path.as_str()).collect();
    picked
        .iter()
        .filter(|entry| match &entry.pass_of {
            None => true,
            Some(lead) => !held.contains(crate::paths::join(&crate::paths::parent(&entry.path), lead).as_str()),
        })
        .map(|e| (*e).clone())
        .collect()
}
