//! The roll's draft, one in each folder of its frames beside other rolls'.

use super::*;
use oni_library::paths;
use oni_settings::prepare::{self, Base, Draft};
use std::collections::BTreeMap;

/// Each frame's draft on entering: its folder's that lists it, or its own sidecar's while that is of an
/// earlier format; the id most frames carry alone resumes, others left out.
pub(super) fn found(paths: &[String]) -> Vec<Option<Draft>> {
    let mut names: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in paths {
        // Entering a roll brings its frames first, so the drafts its steps rewrite are their folders' alone.
        crate::migration::gated(path);
        names.entry(paths::parent(path)).or_default().push(paths::name(path));
    }
    let folders: BTreeMap<String, Vec<Draft>> = names.into_iter().map(|(folder, names)| (folder.clone(), oni_settings::migrate::draft::found(Path::new(&folder), &names))).collect();
    let drafts: Vec<Option<&Draft>> = paths.iter().map(|p| folders[&paths::parent(p)].iter().find(|d| d.frames.contains_key(&paths::name(p)))).collect();
    let lead = most(drafts.iter().flatten().map(|d| d.id));
    drafts.into_iter().map(|d| d.filter(|d| Some(d.id) == lead).cloned()).collect()
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

/// Each folder's part written; the failures' messages.
pub(super) fn written(drafts: Drafts) -> Vec<String> {
    drafts
        .into_iter()
        .filter_map(|part| prepare::write(Path::new(&part.folder), &part.frames, part.draft.as_ref()).err())
        .map(|e| oni_library::refusal::said(&e))
        .collect()
}

#[cfg(test)]
mod tests;
