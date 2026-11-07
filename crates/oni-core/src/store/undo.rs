//! Undo and redo on the session's one history, whichever frames its step wrote and wherever the
//! hand stands, the gallery or the editor on any frame.

use super::*;
use oni_library::roll_folder::Walk;

impl EditorStore {
    pub fn undo(&mut self) -> FieldSet {
        // A step still waiting is recorded first, so the undo takes it back and not the one before.
        self.release();
        match self.history.undo_step() {
            Some(step) => self.take_back(step, true),
            None => FieldSet::EMPTY,
        }
    }

    pub fn redo(&mut self) -> FieldSet {
        self.release();
        match self.history.redo_step() {
            Some(step) => self.take_back(step, false),
            None => FieldSet::EMPTY,
        }
    }

    /// A step that moved files is handed to the host as a walk, which runs it off its interface's
    /// thread, the open frame written first; the rest is laid here.
    pub(super) fn take_back(&mut self, step: Step, undoing: bool) -> FieldSet {
        if let Some(disk) = step.disk {
            // A refused write keeps the files where they are, or the open frame's edits would stay behind.
            let open = self.path().map(str::to_string);
            let moved = |p: &str| disk.moves.iter().any(|m| oni_library::paths::holds(&m.from, p) || oni_library::paths::holds(&m.to, p));
            if let Some(error) = self.flush()
                && open.as_deref().is_some_and(moved)
            {
                self.history.give_back(undoing);
                self.failure = Some(oni_i18n::tr!("Nothing was moved, as the open photo's settings could not be saved: {error}", error = error));
                return FieldSet::EMPTY;
            }
            self.walking = Some(Walk { disk, undoing });
            return FieldSet::EMPTY;
        }
        if let Some(marks) = &step.trash {
            self.lay_trash(marks, undoing);
        }
        if let Some(medals) = &step.medals {
            self.lay_medals(medals, undoing);
        }
        if let Some(m) = &step.marker {
            let film = if undoing { &m.before } else { &m.after };
            match oni_library::folders::set_roll(film.as_ref(), &m.folder) {
                Ok(()) => self.marked = Some((m.folder.clone(), film.clone())),
                Err(e) => self.failure = Some(oni_i18n::tr!("The roll could not be written in its folder: {error}", error = refusal::cause(&e))),
            }
        }
        self.write_back(if undoing { &step.before } else { &step.after })
    }

    /// A walk that moved nothing gives its step back, so the same key offers it again.
    pub fn walk_refused(&mut self, undoing: bool) {
        self.history.give_back(undoing);
    }

    /// Lays restored settings back: the open frame in memory, so the panel never lags, its next step
    /// starting there; the others through their sidecars.
    pub(super) fn write_back(&mut self, restored: &Settings) -> FieldSet {
        for id in restored.values().filter_map(|s| s.roll_reference.as_ref().map(|r| r.id)) {
            self.measured.remove(&RollKey::Measured(id));
        }
        let open = self.path().map(str::to_string);
        let mut changed = FieldSet::EMPTY;
        if let Some(wanted) = open.as_ref().and_then(|p| restored.get(p)) {
            changed = self.replace(wanted.clone());
            self.recorded = wanted.clone();
            self.last_change = Some(self.now());
        }
        let others: Vec<String> = restored.keys().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        self.modify(&others, |path, _| restored.get(path).cloned());
        // The view turns to the face the step was taken on, where what it lays back shows.
        match open.and_then(|p| restored.get(&p)).map(|s| s.face).filter(|f| *f != self.face()) {
            Some(face) => changed.union(self.turn(face)),
            None => changed,
        }
    }
}
