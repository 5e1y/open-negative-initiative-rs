//! The roll's draft, one in each folder of its frames beside other rolls'; a frame's draft of format
//! 21 and under is read where it is fresher than its folder's file, then cleared at the next step's write.

use super::*;
use oni_library::paths;
use oni_settings::prepare::{self, Base, Draft};
use std::collections::BTreeMap;
use std::time::SystemTime;

/// What a frame lands with: the draft that lists it, and whether its own sidecar still keeps one.
pub(in crate::app) struct Found {
    pub draft: Option<Draft>,
    pub stored: bool,
}

/// Each frame's draft on entering, `loaded` its sidecar's reading: its folder's that lists it, else its
/// own, which wins once written after that file; the id most frames carry alone resumes, others left out.
pub(super) fn found(paths: &[String], loaded: &[Option<sidecar::Loaded>]) -> Vec<Found> {
    let mut folders: BTreeMap<String, (Vec<Draft>, Option<SystemTime>)> = BTreeMap::new();
    for path in paths {
        let folder = paths::parent(path);
        let file = prepare::path(Path::new(&folder));
        folders.entry(folder.clone()).or_insert_with(|| (prepare::read(Path::new(&folder)), modified(&file)));
    }
    // A sidecar's own draft wins where only an earlier build writes one after the folder's file.
    let fresher = |p: &String| {
        let (drafts, written) = &folders[&paths::parent(p)];
        !drafts.iter().any(|d| d.frames.contains_key(&paths::name(p))) || modified(&sidecar::url_for(Path::new(p))) > *written
    };
    let own: Vec<Option<&sidecar::RollDraft>> = paths.iter().zip(loaded).map(|(p, l)| l.as_ref()?.roll_draft.as_ref().filter(|_| fresher(p))).collect();
    let mut gathered: BTreeMap<String, Vec<Draft>> = BTreeMap::new();
    for folder in folders.keys() {
        let here = paths.iter().zip(&own).filter(|(p, _)| paths::parent(p) == *folder);
        let kept = here.filter_map(|(p, d)| Some((paths::name(p), (*d)?))).collect::<Vec<_>>();
        gathered.insert(folder.clone(), oni_settings::migrate::draft::gathered(folder, kept.iter().map(|(n, d)| (n.as_str(), *d))));
    }
    let drafts: Vec<Option<&Draft>> = paths
        .iter()
        .zip(&own)
        .map(|(p, d)| {
            let (folder, name) = (paths::parent(p), paths::name(p));
            let pool = if d.is_some() { &gathered[&folder] } else { &folders[&folder].0 };
            pool.iter().find(|d| d.frames.contains_key(&name))
        })
        .collect();
    let lead = most(drafts.iter().flatten().map(|d| d.id));
    let found = drafts.into_iter().zip(loaded).map(|(d, l)| Found {
        draft: d.filter(|d| Some(d.id) == lead).cloned(),
        stored: l.as_ref().is_some_and(|l| l.roll_draft.is_some()),
    });
    found.collect()
}

/// The id met most often, a tie going to the first met.
fn most(ids: impl Iterator<Item = Id>) -> Option<Id> {
    let mut counts: Vec<(Id, usize)> = Vec::new();
    for id in ids {
        match counts.iter_mut().find(|(i, _)| *i == id) {
            Some((_, n)) => *n += 1,
            None => counts.push((id, 1)),
        }
    }
    let top = counts.iter().map(|(_, n)| *n).max()?;
    counts.into_iter().find(|(_, n)| *n == top).map(|(id, _)| id)
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The frame `base` names in the folder of `path`, the frame that landed with its draft.
fn base_at(base: &Base, path: &str, frames: &[Frame]) -> Option<BasePick> {
    let wanted = paths::join(&paths::parent(path), &base.file);
    let frame = frames.iter().find(|f| paths::same(&f.path, &wanted))?;
    Some(BasePick { path: frame.path.clone(), unit: (base.x, base.y), radius: base.radius })
}

impl Roll {
    /// `resume(_:at:)`: the first draft found sets where the roll stands, its files grouped as it says;
    /// each frame then takes its place in its folder's, one of none or of another's left out. True for the first.
    pub(super) fn resume(&mut self, draft: Option<&Draft>, path: &str, known: &passes::Known) -> bool {
        let first = draft.filter(|_| self.draft_id.is_none());
        if let Some(d) = first {
            self.draft_id = Some(d.id);
            self.step = Step::named(&d.step).unwrap_or(self.step);
            self.set_film(d.film);
            self.flat = d.flat_field.clone().unwrap_or_else(|| self.flat.clone());
            self.base = None;
            self.window = d.window.unwrap_or(self.window);
            let draftless = std::mem::take(&mut self.draftless);
            for f in self.frames.iter_mut().filter(|f| draftless.contains(&f.path)) {
                f.ticked = false;
                f.flagged = false;
            }
            self.draftless = draftless;
            let capture = Capture::from_raw(&d.capture).unwrap_or(self.capture);
            self.group_as(capture, d.pass_order.unwrap_or(self.pass_order), d.pass_offset.unwrap_or(self.pass_offset), known);
        }
        let Some(id) = self.draft_id else {
            self.draftless.insert(path.to_string());
            return false;
        };
        let mine = draft.filter(|d| d.id == id);
        if self.base.is_none()
            && let Some(b) = mine.and_then(|d| d.base.as_ref())
        {
            self.base = base_at(b, path, &self.frames);
        }
        let mark = mine.and_then(|d| d.frames.get(&paths::name(path))).copied().unwrap_or_default();
        if let Some(f) = self.frames.iter_mut().find(|f| f.path == path) {
            (f.ticked, f.flagged) = (mark.ticked, mark.flagged);
        }
        first.is_some()
    }
}

/// Each folder's part written, then the drafts of format 21 its frames keep cleared where it landed or
/// was a removal; the failures' messages.
pub(super) fn written(drafts: Drafts, stored: &[String]) -> Vec<String> {
    let mut refused = Vec::new();
    let mut landed = Vec::new();
    for part in drafts {
        let done = prepare::write(Path::new(&part.folder), &part.frames, part.draft.as_ref());
        if let Err(e) = &done {
            refused.push(oni_library::refusal::said(e));
        }
        if done.is_ok() || part.draft.is_none() {
            landed.push(part.folder);
        }
    }
    for path in stored.iter().filter(|p| landed.contains(&paths::parent(p))) {
        if let Err(e) = sidecar::set_roll_draft(None, Path::new(path), mosaic(path)) {
            refused.push(oni_library::refusal::said(&e));
        }
    }
    refused
}

#[cfg(test)]
mod tests;
