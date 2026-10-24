//! Undo and redo, on the editor's history or the gallery's.

use super::*;
use oni_library::gallery_history::Move;

/// An undo or a redo of a roll on the disk: the moves made, and the folder holding what could not
/// move when some could not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Walked {
    pub made: Vec<Move>,
    pub left_in: Option<String>,
}

impl EditorStore {
    pub fn undo(&mut self, surface: Surface) -> FieldSet {
        match surface {
            Surface::Editor => {
                let others = self.history.undo_companion().cloned();
                let Some(state) = self.history.undo() else { return FieldSet::EMPTY };
                let changed = self.replace(state);
                self.last_change = Some(self.now());
                if let Some(others) = others {
                    self.write_back(&others.before);
                }
                changed
            }
            Surface::Gallery => match self.gallery.undo_step() {
                Some(step) => self.take_back(step, true),
                None => FieldSet::EMPTY,
            },
        }
    }

    pub fn redo(&mut self, surface: Surface) -> FieldSet {
        match surface {
            Surface::Editor => {
                let others = self.history.redo_companion().cloned();
                let Some(state) = self.history.redo() else { return FieldSet::EMPTY };
                let changed = self.replace(state);
                self.last_change = Some(self.now());
                if let Some(others) = others {
                    self.write_back(&others.after);
                }
                changed
            }
            Surface::Gallery => match self.gallery.redo_step() {
                Some(step) => self.take_back(step, false),
                None => FieldSet::EMPTY,
            },
        }
    }

    pub(super) fn take_back(&mut self, step: Step, undoing: bool) -> FieldSet {
        if let Some(disk) = &step.disk {
            let made = if undoing { roll_folder::take_back(disk).0 } else { roll_folder::lay(disk) };
            let missed = disk.moves.len().saturating_sub(made.len());
            if missed > 0 {
                self.failure = Some(roll_folder::shortfall(missed, undoing));
            }
            let left_in = disk.folder.clone().filter(|_| missed > 0);
            self.walked = Some(Walked { made, left_in });
            return FieldSet::EMPTY;
        }
        if let Some(marks) = &step.trash {
            self.lay_trash(marks, undoing);
        }
        self.write_back(if undoing { &step.before } else { &step.after })
    }

    /// Lays restored settings back: the open frame in memory, so the panel never lags, the others
    /// through their sidecars.
    pub(super) fn write_back(&mut self, restored: &Settings) -> FieldSet {
        for id in restored.values().filter_map(|s| s.roll_reference.as_ref().map(|r| r.id)) {
            self.measured.remove(&id);
        }
        let open = self.path().map(str::to_string);
        let mut changed = FieldSet::EMPTY;
        if let Some(wanted) = open.as_ref().and_then(|p| restored.get(p)) {
            changed = self.replace(wanted.clone());
            self.last_change = Some(self.now());
        }
        let others: Vec<String> = restored.keys().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        self.modify(&others, |path, _| restored.get(path).cloned());
        changed
    }
}
