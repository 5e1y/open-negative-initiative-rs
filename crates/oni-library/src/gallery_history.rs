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
        let changed: Settings = before
            .iter()
            .filter(|(path, was)| after.get(*path).is_some_and(|now| now != *was))
            .map(|(p, s)| (p.clone(), s.clone()))
            .collect();
        let trash = trash.filter(|t| !t.paths.is_empty());
        if changed.is_empty() && trash.is_none() {
            return;
        }
        let kept: Settings =
            after.iter().filter(|(p, _)| changed.contains_key(*p)).map(|(p, s)| (p.clone(), s.clone())).collect();
        self.push(Step { before: changed, after: kept, label: label.to_owned(), disk: None, trash });
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
mod tests {
    use super::*;

    fn pair() -> (PipelineSettings, PipelineSettings) {
        let mut neutral = PipelineSettings::default();
        neutral.gains.stops = [0.0, 0.0, 0.0];
        let mut edited = PipelineSettings::default();
        edited.gains.stops = [1.0, 0.0, -1.0];
        (neutral, edited)
    }

    fn map(items: &[(&str, &PipelineSettings)]) -> Settings {
        items.iter().map(|(p, s)| (p.to_string(), (*s).clone())).collect()
    }

    #[test]
    fn a_gesture_undoes_and_redoes_whole_and_an_idle_one_opens_no_step() {
        let (neutral, edited) = pair();
        let mut h = GalleryHistory::default();
        assert!(!h.can_undo() && !h.can_redo());
        h.record(
            &map(&[("/a", &edited), ("/b", &edited)]),
            &map(&[("/a", &neutral), ("/b", &neutral)]),
            "Reset 2 photos",
            None,
        );
        assert_eq!(h.undo_label(), Some("Reset 2 photos"));
        let undone = h.undo().unwrap();
        assert_eq!((undone.len(), undone.get("/a")), (2, Some(&edited)));
        assert!(!h.can_undo() && h.can_redo());
        assert_eq!(h.redo().unwrap().get("/a"), Some(&neutral));

        let mut h2 = GalleryHistory::default();
        h2.record(
            &map(&[("/a", &edited), ("/b", &neutral)]),
            &map(&[("/a", &neutral), ("/b", &neutral)]),
            "Reset",
            None,
        );
        assert_eq!(h2.undo().unwrap().len(), 1, "an unchanged photo stays out of the step");
        // Twin: a gesture with no effect opens no step at all.
        let mut h3 = GalleryHistory::default();
        h3.record(&map(&[("/a", &neutral)]), &map(&[("/a", &neutral)]), "Reset", None);
        assert!(!h3.can_undo());

        let mut h4 = GalleryHistory::default();
        h4.record(&map(&[("/a", &edited)]), &map(&[("/a", &neutral)]), "un", None);
        h4.undo();
        h4.record(&map(&[("/a", &edited)]), &map(&[("/a", &neutral)]), "deux", None);
        assert!(h4.can_undo() && !h4.can_redo(), "a new gesture erases the undone branch");
    }

    #[test]
    fn a_trash_step_comes_back_whole_and_an_empty_one_opens_none() {
        let trash = Trash { paths: BTreeSet::from(["/a".into(), "/b".into()]), marked: true };
        let mut h = GalleryHistory::default();
        h.record_trash(trash.clone(), "Move 2 Photos to ONI Trash");
        assert_eq!(h.undo_step().and_then(|s| s.trash), Some(trash));
        assert!(h.redo_step().and_then(|s| s.trash).is_some_and(|t| t.marked));
        // Twin: a restore that touched nothing opens no step.
        let mut empty = GalleryHistory::default();
        empty.record_trash(Trash { paths: BTreeSet::new(), marked: false }, "Restore");
        assert!(!empty.can_undo());
        let mut reset = GalleryHistory::default();
        reset.record(
            &Settings::new(),
            &Settings::new(),
            "Reset",
            Some(Trash { paths: BTreeSet::from(["/sheet.tif".into()]), marked: true }),
        );
        let mut bare = GalleryHistory::default();
        bare.record(&Settings::new(), &Settings::new(), "Reset", None);
        assert!(reset.can_undo() && !bare.can_undo());
    }

    #[test]
    fn a_folder_renamed_after_a_new_roll_is_where_its_undo_walks_from() {
        let (neutral, edited) = pair();
        let mut h = GalleryHistory::default();
        h.record(&map(&[("/p/2026-10-26/A.NEF", &edited)]), &map(&[("/p/2026-10-26/A.NEF", &neutral)]), "Reset", None);
        h.record(&map(&[("/q/B.NEF", &edited)]), &map(&[("/q/B.NEF", &neutral)]), "Reset", None);
        let roll = Disk {
            folder: Some("/p/2026-10-26".into()),
            moves: vec![Move { from: "/p/A.NEF".into(), to: "/p/2026-10-26/A.NEF".into() }],
            ..Disk::default()
        };
        h.record_disk(roll, "New Roll");
        h.follow(&[Move { from: "/p/2026-10-26".into(), to: "/p/Portra".into() }]);
        assert_eq!(h.undo_moves(), [Move { from: "/p/Portra/A.NEF".into(), to: "/p/A.NEF".into() }]);
        assert_eq!(h.undo_step().and_then(|s| s.disk).and_then(|d| d.folder).as_deref(), Some("/p/Portra"));
        assert_eq!(h.undo_paths(), BTreeSet::from(["/q/B.NEF".to_string()]), "twin: a photo outside the folder keeps its path");
        h.undo_step();
        assert_eq!(h.undo_paths(), BTreeSet::from(["/p/Portra/A.NEF".to_string()]), "a settings step under it follows");
    }

    #[test]
    fn the_stack_keeps_its_last_hundred_steps() {
        let mut h = GalleryHistory::default();
        for i in 0..(LIMIT + 5) {
            h.record_disk(
                Disk { moves: vec![Move { from: format!("/a{i}"), to: "/b".into() }], ..Disk::default() },
                &i.to_string(),
            );
        }
        let mut undone = 0;
        while h.undo_step().is_some() {
            undone += 1;
        }
        assert_eq!(undone, LIMIT);
        // Twin: a disk gesture that moved nothing adds no step.
        let mut idle = GalleryHistory::default();
        idle.record_disk(Disk::default(), "Move");
        assert!(!idle.can_undo());
    }

    #[test]
    fn the_stack_keeps_its_steps_under_a_ceiling_of_bytes() {
        let (neutral, edited) = pair();
        let gesture = |h: &mut GalleryHistory, n: usize| {
            let paths: Vec<String> = (0..n).map(|i| format!("/r/{i}.NEF")).collect();
            let before: Settings = paths.iter().map(|p| (p.clone(), edited.clone())).collect();
            let after: Settings = paths.iter().map(|p| (p.clone(), neutral.clone())).collect();
            h.record(&before, &after, "Reset", None);
        };
        let mut probe = GalleryHistory::default();
        gesture(&mut probe, 50);
        let step = probe.bytes();
        let mut h = GalleryHistory::bounded(step * 3);
        for _ in 0..10 {
            gesture(&mut h, 50);
        }
        let mut undone = 0;
        while h.undo_step().is_some() {
            undone += 1;
        }
        assert_eq!(undone, 3, "three steps of {step} bytes fill the ceiling");
        // The newest step stays, even past the ceiling alone.
        let mut tight = GalleryHistory::bounded(1);
        gesture(&mut tight, 50);
        assert!(tight.can_undo());
        // Twin: counted by steps alone, the ten gestures would all be kept.
        let mut counted = GalleryHistory::bounded(usize::MAX);
        for _ in 0..10 {
            gesture(&mut counted, 50);
        }
        assert_eq!(counted.bytes(), step * 10);
    }
}
