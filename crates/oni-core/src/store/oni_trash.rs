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
        let mosaic = &self.archive.mosaic;
        let outcome = mark_run(paths, date, &|p: &Path| mosaic(p));
        self.mark_landed(paths, &outcome, date)
    }

    /// Before a mark runs off the host's thread: the open frame among `paths` written, and closed
    /// when it goes to the ONI Trash, so no edit lands on it meanwhile; a refused write keeps its edits.
    pub fn mark_begun(&mut self, paths: &[String], date: Option<Date>) {
        if !self.path().is_some_and(|p| paths.iter().any(|t| t == p)) {
            return;
        }
        if date.is_some() {
            self.close();
        } else {
            self.flush();
        }
    }

    /// Once the mark has run: its failures said and one gallery step recorded; the frames marked.
    pub fn mark_landed(&mut self, paths: &[String], outcome: &trash::Outcome, date: Option<Date>) -> usize {
        self.took(outcome, date.is_some());
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
            self.close();
        }
    }
}

/// The mark itself, every pass of a frame of passes with it: sidecar writes only, safe off the
/// interface's thread.
pub fn mark_run(paths: &[String], date: Option<Date>, mosaic: &dyn Fn(&Path) -> bool) -> trash::Outcome {
    trash::mark(&oni_library::rgb_passes::expanded(paths), date, mosaic)
}
