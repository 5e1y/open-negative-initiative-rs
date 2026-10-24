//! A gesture's sidecar writes or file moves run off this thread, one at a time, the window answering
//! meanwhile; another such gesture waits for it, and a quit waits for it too.

use super::*;
use oni_library::roll_folder::{self, Walked};
use std::sync::{Mutex, PoisonError};

/// Held while a gesture's writes or moves run; a quit takes it to wait for the last one.
static RUNNING: Mutex<()> = Mutex::new(());

/// Returns once no gesture's writes or moves run, so a quit never cuts a frame's move halfway.
pub(in crate::app) fn wait_for_disk() {
    drop(RUNNING.lock().unwrap_or_else(PoisonError::into_inner));
}

impl Ctl {
    /// A gesture refused, and said so, while another's writes or moves run.
    pub(in crate::app) fn busy_said(&self, title: &str) -> bool {
        let Some(running) = self.state.borrow().org.disk_busy.clone() else { return false };
        self.told(title, &tr!("“{gesture}” is still under way: try again once it is done.", gesture = running));
        true
    }

    /// `run` off this thread, `busy` on the status bar meanwhile, then `land` back here with what it did.
    pub(in crate::app) fn on_disk<T: Send + 'static>(
        self: &Rc<Self>,
        busy: &str,
        run: impl FnOnce() -> T + Send + 'static,
        land: impl FnOnce(&Rc<Ctl>, T) + Send + 'static,
    ) {
        self.state.borrow_mut().org.disk_busy = Some(busy.to_owned());
        let said = busy.to_owned();
        let spawned = std::thread::Builder::new().name("disk gesture".into()).spawn(move || {
            let outcome = {
                let _turn = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
            };
            with_shell(move |c| {
                c.state.borrow_mut().org.disk_busy = None;
                match outcome {
                    Ok(done) => land(c, done),
                    Err(_) => c.told(&said, tr("It stopped halfway: read the folder again to see where each file is.")),
                }
            });
        });
        if spawned.is_err() {
            self.state.borrow_mut().org.disk_busy = None;
            self.told(busy, tr("It could not start: nothing was changed."));
        }
    }

    /// Marks or clears the ONI Trash's mark on `targets` off this thread, then one gallery step.
    pub(in crate::app) fn mark_off_thread(self: &Rc<Self>, title: &'static str, targets: Vec<String>, date: Option<oni_settings::Date>) {
        if targets.is_empty() || self.busy_said(title) {
            return;
        }
        let mosaic = {
            let mut s = self.state.borrow_mut();
            let Some(store) = s.store.editor() else { return };
            store.mark_begun(&targets, date);
            store.mosaic_rule()
        };
        self.closed_if_trashed();
        let busy = if date.is_some() { tr("Moving to the ONI Trash") } else { tr("Restoring from the ONI Trash") };
        let run = targets.clone();
        self.on_disk(busy, move || oni_core::store::mark_run(&run, date, &|p: &Path| mosaic(p)), move |c, outcome| c.marks_landed(title, &targets, &outcome, date));
    }

    fn marks_landed(self: &Rc<Self>, title: &str, targets: &[String], outcome: &oni_library::trash::Outcome, date: Option<oni_settings::Date>) {
        let (failure, frames, folder) = {
            let mut s = self.state.borrow_mut();
            let folder = s.org.restore_told.take().filter(|(_, asked)| asked == targets).map(|(f, _)| f);
            let Some(store) = s.store.editor() else { return };
            let frames = store.mark_landed(targets, outcome, date);
            (store.failure.take(), frames, folder)
        };
        if let Some(message) = failure {
            self.told(title, &message);
        } else if let Some(folder) = folder.filter(|_| date.is_none() && frames > 0) {
            // A folder row's RESTORE asks nothing first, and its row goes: the notice brings it back.
            let said = trn("One photo of “{folder}” is back in its folder.", "{n} photos of “{folder}” are back in their folder.", frames as u64);
            self.told_step(title, &said.replace("{folder}", &oni_library::paths::name(&folder)));
        }
        self.landed(targets);
        if date.is_none() {
            let gone: HashSet<String> = targets.iter().cloned().collect();
            self.state.borrow_mut().trash_picks.forget(&gone);
            self.lay_grid();
        }
    }

    /// An undo or a redo of a roll, its moves off this thread; a refused one keeps its step.
    pub(in crate::app) fn walk_off_thread(self: &Rc<Self>, title: &'static str, walk: roll_folder::Walk, touched: Vec<String>) {
        let undoing = walk.undoing;
        let busy = if undoing { tr("Putting the photos back") } else { tr("Moving the photos into the roll again") };
        self.on_disk(busy, move || roll_folder::walk(&walk, &roll_folder::rename_new), move |c, walked| c.walked(title, undoing, &touched, walked));
    }

    fn walked(self: &Rc<Self>, title: &str, undoing: bool, touched: &[String], walked: Walked) {
        if walked.refused
            && let Some(store) = self.state.borrow_mut().store.editor()
        {
            store.walk_refused(undoing);
        }
        if let Some(message) = &walked.said {
            self.told_with(title, message, walked.left_in.clone());
        }
        self.landed(touched);
        if !walked.made.is_empty() {
            self.followed(&walked.made);
        }
        self.publish_bar();
    }
}
