//! The ONI Trash: frames marked in their sidecars, their files untouched.

use super::*;

impl EditorStore {
    /// Marks each frame's sidecar, its files untouched; the open frame among them closes.
    pub fn move_to_trash(&mut self, paths: &[String]) -> usize {
        self.mark(paths, Some(Date::now()))
    }

    pub fn restore_from_trash(&mut self, paths: &[String]) -> usize {
        self.mark(paths, None)
    }

    pub(super) fn mark(&mut self, paths: &[String], date: Option<Date>) -> usize {
        if self.path().is_some_and(|p| paths.iter().any(|t| t == p)) {
            self.flush();
        }
        let files = oni_library::rgb_passes::expanded(paths);
        let mosaic = &self.archive.mosaic;
        let outcome = trash::mark(&files, date, &|p: &Path| mosaic(p));
        self.took(&outcome, date.is_some());
        let frames = paths.iter().filter(|p| outcome.done.contains(p)).count();
        let label = if date.is_some() {
            trn("Move to ONI Trash", "Move {n} Photos to ONI Trash", frames as u64)
        } else {
            trn("Restore {n} Photo", "Restore {n} Photos", frames as u64)
        };
        let marks = Trash { paths: outcome.done.iter().cloned().collect(), marked: date.is_some() };
        self.gallery.record_trash(marks, &label);
        frames
    }

    pub(super) fn lay_trash(&mut self, marks: &Trash, undoing: bool) {
        let date = (marks.marked != undoing).then(Date::now);
        let files: Vec<String> = marks.paths.iter().cloned().collect();
        if date.is_some() && self.path().is_some_and(|p| marks.paths.contains(p)) {
            self.flush();
        }
        let mosaic = &self.archive.mosaic;
        let outcome = trash::mark(&files, date, &|p: &Path| mosaic(p));
        self.took(&outcome, date.is_some());
    }

    pub(super) fn took(&mut self, outcome: &trash::Outcome, marked: bool) {
        if let Some(message) = trash::report(outcome, !marked) {
            self.failure = Some(message);
        }
        if marked && self.path().is_some_and(|p| outcome.done.iter().any(|d| d == p)) {
            self.open = None;
            self.history = History::default();
            self.replace(PipelineSettings::default());
        }
    }
}
