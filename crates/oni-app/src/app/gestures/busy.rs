//! A gesture's sidecar writes or file moves run off this thread, one at a time, the window answering
//! meanwhile; another such gesture waits for it, and a quit waits for it too.

use super::*;
use oni_library::roll_folder::{self, Walked};
use std::sync::{Condvar, Mutex, PoisonError};

/// Held while a gesture's writes or moves run, so they run one at a time.
static RUNNING: Mutex<()> = Mutex::new(());

/// The gestures handed to a thread and not yet run, counted before the thread starts, so a quit
/// landing first still waits for them.
static IN_FLIGHT: Mutex<usize> = Mutex::new(0);
static LANDED: Condvar = Condvar::new();

/// Returns once no gesture's writes or moves run nor wait to, so a quit never cuts a frame's move halfway.
pub(in crate::app) fn wait_for_disk() {
    let mut flying = IN_FLIGHT.lock().unwrap_or_else(PoisonError::into_inner);
    while *flying > 0 {
        flying = LANDED.wait(flying).unwrap_or_else(PoisonError::into_inner);
    }
}

/// One gesture counted in flight until dropped, its thread's run done or its spawn failed.
struct Flight;

impl Flight {
    fn new() -> Flight {
        *IN_FLIGHT.lock().unwrap_or_else(PoisonError::into_inner) += 1;
        Flight
    }
}

impl Drop for Flight {
    fn drop(&mut self) {
        *IN_FLIGHT.lock().unwrap_or_else(PoisonError::into_inner) -= 1;
        LANDED.notify_all();
    }
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
        self.reads_held();
        let said = busy.to_owned();
        let flight = Flight::new();
        let spawned = std::thread::Builder::new().name("disk gesture".into()).spawn(move || {
            let outcome = {
                let _turn = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));
                drop(flight);
                outcome
            };
            with_shell(move |c| {
                c.state.borrow_mut().org.disk_busy = None;
                match outcome {
                    Ok(done) => land(c, done),
                    Err(_) => c.told(&said, tr("It stopped halfway: read the folder again to see where each file is.")),
                }
                c.reads_freed();
            });
        });
        if spawned.is_err() {
            self.state.borrow_mut().org.disk_busy = None;
            self.told(busy, tr("It could not start: nothing was changed."));
            self.reads_freed();
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
        let (run, apart) = (targets.clone(), self.reread_apart());
        let marked = move || {
            let outcome = oni_core::store::mark_run(&run, date, &|p: &Path| mosaic(p));
            (outcome, apart(&run))
        };
        self.on_disk(busy, marked, move |c, (outcome, reread)| c.marks_landed(title, &targets, &outcome, date, reread));
    }

    /// `library::reread` for a gesture's thread, on a connection of its own, of the folder open now.
    fn reread_apart(&self) -> impl FnOnce(&[String]) -> library::Reread + Send + 'static {
        let (index_path, opened) = {
            let s = self.state.borrow();
            (s.index_path.clone(), s.opened.clone())
        };
        move |paths| LibraryIndex::open(&index_path).map(|index| library::reread(&index, paths, opened.as_deref())).unwrap_or_default()
    }

    fn marks_landed(self: &Rc<Self>, title: &str, targets: &[String], outcome: &oni_library::trash::Outcome, date: Option<oni_settings::Date>, reread: library::Reread) {
        let (failure, frames, folder) = {
            let mut s = self.state.borrow_mut();
            let folder = s.org.restore_told.take().filter(|(_, asked)| asked == targets).map(|(f, _)| f);
            let Some(store) = s.store.editor() else { return };
            let frames = store.mark_landed(targets, outcome, date);
            (store.failure.take(), frames, folder)
        };
        if let Some(message) = failure {
            self.told(title, &message);
        } else if date.is_some() && frames > 0 {
            // A photo leaves its folder's view: the notice says so, its Undo bringing it back.
            self.told_step(title, &trn("One photo moved to the ONI Trash.", "{n} photos moved to the ONI Trash.", frames as u64));
        } else if let Some(folder) = folder.filter(|_| date.is_none() && frames > 0) {
            // A folder row's RESTORE asks nothing first, and its row goes: the notice brings it back.
            let said = trn("One photo of “{folder}” is back in its folder.", "{n} photos of “{folder}” are back in their folder.", frames as u64);
            self.told_step(title, &said.replace("{folder}", &oni_library::paths::name(&folder)));
        }
        self.landed_with(targets, Some(reread));
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
        let (read, apart) = (touched.clone(), self.reread_apart());
        let run = move || {
            let walked = roll_folder::walk(&walk, &roll_folder::rename_new);
            (walked, apart(&read))
        };
        self.on_disk(busy, run, move |c, (walked, reread)| c.walked(title, undoing, &touched, walked, reread));
    }

    fn walked(self: &Rc<Self>, title: &str, undoing: bool, touched: &[String], walked: Walked, reread: library::Reread) {
        if walked.refused
            && let Some(store) = self.state.borrow_mut().store.editor()
        {
            store.walk_refused(undoing);
        }
        if let Some(message) = &walked.said {
            self.told_with(title, message, walked.left_in.clone());
        }
        self.landed_with(touched, Some(reread));
        if !walked.made.is_empty() {
            self.followed(&walked.made);
        }
        self.publish_bar();
    }
}
