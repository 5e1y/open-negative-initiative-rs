//! The session's one undo history, the same in the gallery and the editor: every gesture is one step
//! naming each photo it wrote, an edit of the open frame as much as a gesture on several.

use crate::paths;
use oni_settings::{Film, Id, Medal, PipelineSettings};
use std::collections::{BTreeMap, BTreeSet};

mod class;
pub use class::Class;

pub type Settings = BTreeMap<String, PipelineSettings>;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Step {
    pub before: Settings,
    pub after: Settings,
    pub label: String,
    pub disk: Option<Disk>,
    pub trash: Option<Trash>,
    pub marker: Option<Marker>,
    pub medals: Option<Medals>,
    pub members: Option<Members>,
}

/// A folder's roll marker as the step found it and as it left it; `None` is a plain folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub folder: String,
    pub before: Option<Film>,
    pub after: Option<Film>,
}

/// Medals a gesture laid: the one each photo wore before it, and the one they all took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Medals {
    pub before: BTreeMap<String, Medal>,
    pub after: Medal,
}

/// The projects whose members a gesture changed, each one's list as the step found it and left it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Members {
    pub before: BTreeMap<Id, Vec<String>>,
    pub after: BTreeMap<Id, Vec<String>>,
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
    /// Every photo the step writes, marks or moves, at either end of a move; a project's members
    /// change no photo.
    pub fn paths(&self) -> BTreeSet<String> {
        let mut out: BTreeSet<String> = self.before.keys().chain(self.after.keys()).cloned().collect();
        out.extend(self.trash.iter().flat_map(|t| t.paths.iter().cloned()));
        out.extend(self.medals.iter().flat_map(|m| m.before.keys().cloned()));
        out.extend(self.disk.iter().flat_map(|d| d.moves.iter().flat_map(|m| [m.from.clone(), m.to.clone()])));
        out
    }
}

/// Below this interval, two edits of the same setting on the same frame are one step.
pub const COALESCE_WINDOW: f64 = 0.4;
pub const LIMIT: usize = 200;
/// The settings the steps hold, at most: a gesture on thousands of photos keeps two whole settings
/// each, and a stack of them would hold gigabytes. The newest step stays whatever its size.
pub const BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct History {
    steps: Vec<Step>,
    /// Each step's settings in bytes, as their JSON spells them.
    sizes: Vec<usize>,
    cursor: usize,
    ceiling: usize,
    /// The frame, the setting and the time of the edit on top, which an edit continuing it joins.
    last: Option<(String, Class, f64)>,
}

impl Default for History {
    fn default() -> Self {
        History::bounded(BYTES)
    }
}

/// A step's settings as their JSON spells them, near what their clones hold.
fn weight(step: &Step) -> usize {
    let one = |s: &PipelineSettings| serde_json::to_vec(s).map_or(0, |v| v.len());
    step.before.values().chain(step.after.values()).map(one).sum()
}

impl History {
    /// A history holding at most `ceiling` bytes of settings.
    pub fn bounded(ceiling: usize) -> History {
        History { steps: Vec::new(), sizes: Vec::new(), cursor: 0, ceiling, last: None }
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

    /// The step the next undo takes back, left in place, which a host names when it carries no label.
    pub fn next_undo(&self) -> Option<&Step> {
        self.cursor.checked_sub(1).and_then(|at| self.steps.get(at))
    }

    pub fn next_redo(&self) -> Option<&Step> {
        self.steps.get(self.cursor)
    }

    /// The name of the next undo's gesture; an edit of the open frame has none.
    pub fn undo_label(&self) -> Option<&str> {
        self.next_undo().map(|s| s.label.as_str()).filter(|l| !l.is_empty())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.next_redo().map(|s| s.label.as_str()).filter(|l| !l.is_empty())
    }

    /// The photos the next undo would touch, so a host reads their rows and thumbnails again.
    pub fn undo_paths(&self) -> BTreeSet<String> {
        self.next_undo().map(Step::paths).unwrap_or_default()
    }

    pub fn redo_paths(&self) -> BTreeSet<String> {
        self.next_redo().map(Step::paths).unwrap_or_default()
    }

    /// A new gesture drops the branch that was undone, and the oldest steps past the limit or the
    /// ceiling.
    fn push(&mut self, step: Step) {
        self.last = None;
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

    /// Records an edit of the open frame `path`, with the other frames it wrote; it joins the step on
    /// top while the same gesture goes on there, never one an undo left, so the past is never rewritten.
    pub fn record_edit(&mut self, path: &str, before: Settings, after: Settings, now: f64) {
        let class = match (before.get(path), after.get(path)) {
            (Some(was), Some(is)) => Class::between(was, is),
            _ => Class::Several,
        };
        let joins = self.cursor == self.steps.len()
            && class != Class::Several
            && self.last.as_ref().is_some_and(|(p, c, t)| p == path && *c == class && now - t < COALESCE_WINDOW);
        if joins {
            let top = self.steps.last_mut().expect("the edit on top");
            for (p, was) in before {
                top.before.entry(p).or_insert(was);
            }
            top.after.extend(after);
            let at = self.sizes.len() - 1;
            self.sizes[at] = weight(&self.steps[at]);
        } else if !self.record_with(&before, &after, "", None, None) {
            return;
        }
        self.last = Some((path.to_owned(), class, now));
    }

    /// Whether a step was laid.
    fn record_with(&mut self, before: &Settings, after: &Settings, label: &str, trash: Option<Trash>, marker: Option<Marker>) -> bool {
        let changed: Settings = before
            .iter()
            .filter(|(path, was)| after.get(*path).is_some_and(|now| now != *was))
            .map(|(p, s)| (p.clone(), s.clone()))
            .collect();
        let trash = trash.filter(|t| !t.paths.is_empty());
        let marker = marker.filter(|m| m.before != m.after);
        if changed.is_empty() && trash.is_none() && marker.is_none() {
            return false;
        }
        let kept: Settings =
            after.iter().filter(|(p, _)| changed.contains_key(*p)).map(|(p, s)| (p.clone(), s.clone())).collect();
        self.push(Step { before: changed, after: kept, label: label.to_owned(), trash, marker, ..Step::default() });
        true
    }

    /// Records a gesture on the disk, already applied; one that moved nothing opens no step.
    pub fn record_disk(&mut self, disk: Disk, label: &str) {
        if disk.moves.is_empty() {
            return;
        }
        self.push(Step { label: label.to_owned(), disk: Some(disk), ..Step::default() });
    }

    /// Records a mark or a restore, already applied; one that touched nothing opens no step.
    pub fn record_trash(&mut self, trash: Trash, label: &str) {
        if trash.paths.is_empty() {
            return;
        }
        self.push(Step { label: label.to_owned(), trash: Some(trash), ..Step::default() });
    }

    /// Records medals laid, already written; a photo that wore the medal already is left out, and
    /// a gesture that changed none opens no step.
    pub fn record_medals(&mut self, mut medals: Medals, label: &str) {
        medals.before.retain(|_, worn| *worn != medals.after);
        if medals.before.is_empty() {
            return;
        }
        self.push(Step { label: label.to_owned(), medals: Some(medals), ..Step::default() });
    }

    /// Records the projects' members a gesture changed, already saved; a project left as it was is
    /// left out, and a gesture that changed none opens no step.
    pub fn record_members(&mut self, mut members: Members, label: &str) {
        let changed: Vec<Id> = members.after.iter().filter(|(id, after)| members.before.get(*id) != Some(*after)).map(|(id, _)| *id).collect();
        members.before.retain(|id, _| changed.contains(id));
        members.after.retain(|id, _| changed.contains(id));
        if changed.is_empty() {
            return;
        }
        self.push(Step { label: label.to_owned(), members: Some(members), ..Step::default() });
    }

    /// A walk refused before anything moved gives its step back, so the same key offers it again.
    pub fn give_back(&mut self, undoing: bool) {
        self.last = None;
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
            if let Some(medals) = &mut step.medals {
                medals.before = std::mem::take(&mut medals.before).into_iter().map(|(p, m)| (moved(&p), m)).collect();
            }
            if let Some(members) = &mut step.members {
                for list in members.before.values_mut().chain(members.after.values_mut()) {
                    list.iter_mut().for_each(|p| *p = moved(p));
                }
            }
            if let Some(disk) = &mut step.disk {
                disk.folder = disk.folder.as_deref().map(moved);
                for m in &mut disk.moves {
                    *m = Move { from: moved(&m.from), to: moved(&m.to) };
                }
            }
        }
    }

    /// The step on top, taken back; the next edit never joins the state just left.
    pub fn undo_step(&mut self) -> Option<Step> {
        if !self.can_undo() {
            return None;
        }
        self.last = None;
        self.cursor -= 1;
        Some(self.steps[self.cursor].clone())
    }

    pub fn redo_step(&mut self) -> Option<Step> {
        if !self.can_redo() {
            return None;
        }
        self.last = None;
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
