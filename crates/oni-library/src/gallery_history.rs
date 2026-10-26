//! Undo for gestures that act on several photographs at once, from the gallery. Kept apart from
//! the open photo's history, so ⌘Z answers for the right stack.

use crate::paths;
use oni_settings::{Film, PipelineSettings};
use std::collections::{BTreeMap, BTreeSet};

pub type Settings = BTreeMap<String, PipelineSettings>;

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub before: Settings,
    pub after: Settings,
    pub label: String,
    pub disk: Option<Disk>,
    pub trash: Option<Trash>,
    pub marker: Option<Marker>,
}

/// A folder's roll marker as the step found it and as it left it; `None` is a plain folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub folder: String,
    pub before: Option<Film>,
    pub after: Option<Film>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trash {
    pub paths: BTreeSet<String>,
    pub marked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Move {
    pub from: String,
    pub to: String,
}

/// Items moved on the disk, each from its old path to its new one, and the roll's folder made
/// for them when the gesture made one.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Disk {
    pub folder: Option<String>,
    pub film: Option<Film>,
    pub moves: Vec<Move>,
    /// How many of `moves`, in order, make each frame that moves whole or not at all; none, one each.
    pub units: Vec<usize>,
}

impl Step {
    /// Every photo the step writes, marks or moves, at either end of a move.
    pub fn paths(&self) -> BTreeSet<String> {
        let mut out: BTreeSet<String> = self.before.keys().chain(self.after.keys()).cloned().collect();
        out.extend(self.trash.iter().flat_map(|t| t.paths.iter().cloned()));
        out.extend(self.disk.iter().flat_map(|d| d.moves.iter().flat_map(|m| [m.from.clone(), m.to.clone()])));
        out
    }
}

pub const LIMIT: usize = 100;
/// The settings the steps hold, at most: a gesture on thousands of photos keeps two whole settings
/// each, and a hundred of them would hold gigabytes. The newest step stays whatever its size.
pub const BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct GalleryHistory {
    steps: Vec<Step>,
    /// Each step's settings in bytes, as their JSON spells them.
    sizes: Vec<usize>,
    cursor: usize,
    ceiling: usize,
}

impl Default for GalleryHistory {
    fn default() -> Self {
        GalleryHistory::bounded(BYTES)
    }
}

/// A step's settings as their JSON spells them, near what their clones hold.
fn weight(step: &Step) -> usize {
    let one = |s: &PipelineSettings| serde_json::to_vec(s).map_or(0, |v| v.len());
    step.before.values().chain(step.after.values()).map(one).sum()
}

impl GalleryHistory {
    /// A history holding at most `ceiling` bytes of settings.
    pub fn bounded(ceiling: usize) -> GalleryHistory {
        GalleryHistory { steps: Vec::new(), sizes: Vec::new(), cursor: 0, ceiling }
    }

    /// The bytes of settings the steps hold now.
    pub fn bytes(&self) -> usize {
        self.sizes.iter().sum()
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.steps.len()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.can_undo().then(|| self.steps[self.cursor - 1].label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.can_redo().then(|| self.steps[self.cursor].label.as_str())
    }

    /// The photos the next undo would touch, so a host reads their rows and thumbnails again.
    pub fn undo_paths(&self) -> BTreeSet<String> {
        self.can_undo().then(|| self.steps[self.cursor - 1].paths()).unwrap_or_default()
    }

    pub fn redo_paths(&self) -> BTreeSet<String> {
        self.can_redo().then(|| self.steps[self.cursor].paths()).unwrap_or_default()
    }

    /// The files the next undo would move, each as the undo moves it, so a host follows them.
    pub fn undo_moves(&self) -> Vec<Move> {
        let step = self.can_undo().then(|| &self.steps[self.cursor - 1]);
        let moves = step.and_then(|s| s.disk.as_ref()).map(|d| d.moves.clone()).unwrap_or_default();
        moves.into_iter().rev().map(|m| Move { from: m.to, to: m.from }).collect()
    }

    pub fn redo_moves(&self) -> Vec<Move> {
        let step = self.can_redo().then(|| &self.steps[self.cursor]);
        step.and_then(|s| s.disk.as_ref()).map(|d| d.moves.clone()).unwrap_or_default()
    }

    /// A new gesture drops the branch that was undone, and the oldest steps past the limit or the
    /// ceiling.
    fn push(&mut self, step: Step) {
        self.steps.truncate(self.cursor);
        self.sizes.truncate(self.cursor);
        self.sizes.push(weight(&step));
        self.steps.push(step);
        let mut over = self.steps.len().saturating_sub(LIMIT);
        let mut held: usize = self.sizes[over..].iter().sum();
        while held > self.ceiling && over + 1 < self.steps.len() {
            held -= self.sizes[over];
            over += 1;
        }
        self.steps.drain(..over);
        self.sizes.drain(..over);
        self.cursor = self.steps.len();
    }

    /// Records a gesture already applied, with what it sent to the ONI Trash; a photo it left
    /// unchanged is dropped, so undoing rewrites and re-thumbnails nothing already right.
    pub fn record(&mut self, before: &Settings, after: &Settings, label: &str, trash: Option<Trash>) {
        self.record_with(before, after, label, trash, None);
    }

    /// `record`, with the roll marker the gesture wrote; a marker left as it was opens no step.
    pub fn record_marked(&mut self, before: &Settings, after: &Settings, label: &str, marker: Option<Marker>) {
        self.record_with(before, after, label, None, marker);
    }

    fn record_with(&mut self, before: &Settings, after: &Settings, label: &str, trash: Option<Trash>, marker: Option<Marker>) {
        let changed: Settings = before
            .iter()
            .filter(|(path, was)| after.get(*path).is_some_and(|now| now != *was))
            .map(|(p, s)| (p.clone(), s.clone()))
            .collect();
        let trash = trash.filter(|t| !t.paths.is_empty());
        let marker = marker.filter(|m| m.before != m.after);
        if changed.is_empty() && trash.is_none() && marker.is_none() {
            return;
        }
        let kept: Settings =
            after.iter().filter(|(p, _)| changed.contains_key(*p)).map(|(p, s)| (p.clone(), s.clone())).collect();
        self.push(Step { before: changed, after: kept, label: label.to_owned(), disk: None, trash, marker });
    }

    /// Records a gesture on the disk, already applied; one that moved nothing opens no step.
    pub fn record_disk(&mut self, disk: Disk, label: &str) {
        if disk.moves.is_empty() {
            return;
        }
        self.push(Step {
            before: Settings::new(),
            after: Settings::new(),
            label: label.to_owned(),
            disk: Some(disk),
            trash: None,
            marker: None,
        });
    }

    /// Records a mark or a restore, already applied; one that touched nothing opens no step.
    pub fn record_trash(&mut self, trash: Trash, label: &str) {
        if trash.paths.is_empty() {
            return;
        }
        self.push(Step {
            before: Settings::new(),
            after: Settings::new(),
            label: label.to_owned(),
            disk: None,
            trash: Some(trash),
            marker: None,
        });
    }

    /// A walk refused before anything moved gives its step back, so the same key offers it again.
    pub fn give_back(&mut self, undoing: bool) {
        if undoing && self.can_redo() {
            self.cursor += 1;
        } else if !undoing && self.can_undo() {
            self.cursor -= 1;
        }
    }

    /// Every step names its photos and folders where `moves` sent them, so an undo after a folder
    /// is renamed reaches the files under their new name.
    pub fn follow(&mut self, moves: &[Move]) {
        let moved = |path: &str| {
            moves.iter().find(|m| paths::holds(&m.from, path)).map_or_else(|| path.to_owned(), |m| paths::moved(path, &m.from, &m.to))
        };
        let rekeyed = |held: &mut Settings| *held = std::mem::take(held).into_iter().map(|(p, s)| (moved(&p), s)).collect();
        for step in &mut self.steps {
            rekeyed(&mut step.before);
            rekeyed(&mut step.after);
            if let Some(trash) = &mut step.trash {
                trash.paths = trash.paths.iter().map(|p| moved(p)).collect();
            }
            if let Some(marker) = &mut step.marker {
                marker.folder = moved(&marker.folder);
            }
            if let Some(disk) = &mut step.disk {
                disk.folder = disk.folder.as_deref().map(moved);
                for m in &mut disk.moves {
                    *m = Move { from: moved(&m.from), to: moved(&m.to) };
                }
            }
        }
    }

    pub fn undo_step(&mut self) -> Option<Step> {
        if !self.can_undo() {
            return None;
        }
        self.cursor -= 1;
        Some(self.steps[self.cursor].clone())
    }

    pub fn redo_step(&mut self) -> Option<Step> {
        if !self.can_redo() {
            return None;
        }
        self.cursor += 1;
        Some(self.steps[self.cursor - 1].clone())
    }

    pub fn undo(&mut self) -> Option<Settings> {
        self.undo_step().map(|s| s.before)
    }

    pub fn redo(&mut self) -> Option<Settings> {
        self.redo_step().map(|s| s.after)
    }
}

#[cfg(test)]
mod tests;
