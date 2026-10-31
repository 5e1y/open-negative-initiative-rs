//! Undo and redo, on the editor's history or the gallery's.

use super::*;
use oni_library::roll_folder::Walk;

impl EditorStore {
    pub fn undo(&mut self, surface: Surface) -> FieldSet {
        // A step still waiting is recorded first, so the undo takes it back and not the one before.
        self.release();
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
        self.release();
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

    /// A step that moved files is handed to the host as a walk, which runs it off its interface's
    /// thread; the rest is laid here.
    pub(super) fn take_back(&mut self, step: Step, undoing: bool) -> FieldSet {
        if let Some(disk) = step.disk {
            self.walking = Some(Walk { disk, undoing });
            return FieldSet::EMPTY;
        }
        if let Some(marks) = &step.trash {
            self.lay_trash(marks, undoing);
        }
        if let Some(m) = &step.marker {
            let film = if undoing { &m.before } else { &m.after };
            match oni_library::folders::set_roll(film.as_ref(), &m.folder) {
                Ok(()) => self.marked = Some((m.folder.clone(), film.clone())),
                Err(e) => self.failure = Some(oni_i18n::tr!("The roll could not be written in its folder: {error}", error = refusal::cause(&e))),
            }
        }
        let changed = self.write_back(if undoing { &step.before } else { &step.after });
        // The open frame laid back is an editor step too, as Swift records every change but a replay:
        // its next edit never starts from the state just undone.
        if !changed.is_empty() {
            let now = self.now();
            self.history.record_apart(self.settings.clone(), now);
        }
        changed
    }

    /// A walk that moved nothing gives its step back, so the same key offers it again.
    pub fn walk_refused(&mut self, undoing: bool) {
        self.gallery.give_back(undoing);
    }

    /// Lays restored settings back: the open frame in memory, so the panel never lags, the others
    /// through their sidecars.
    pub(super) fn write_back(&mut self, restored: &Settings) -> FieldSet {
        for id in restored.values().filter_map(|s| s.roll_reference.as_ref().map(|r| r.id)) {
            self.measured.remove(&RollKey::Measured(id));
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
