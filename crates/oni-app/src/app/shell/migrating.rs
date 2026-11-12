//! The sweep seen from the window: the sheet said once, the line while it runs, the notice at its end
//! when files were left as they were; started at the launch and again when a disk comes back.

use super::*;
use crate::migration::{self, Place, Sweep};
use crate::migration::ledger::Why;
use crate::Upgraded;
use std::sync::atomic::Ordering;

/// How often the line follows the sweep.
const WATCH: Duration = Duration::from_millis(250);

/// The caches the launch named, and the sweep under way with the timer that says how it stands; the
/// interface thread's alone.
#[derive(Default)]
struct Sweeping {
    caches: Option<PathBuf>,
    running: Option<(Sweep, Timer)>,
}

thread_local! {
    static SWEEPING: RefCell<Sweeping> = RefCell::default();
}

/// The sweep stopped within one file and joined, as the app quits.
pub(in crate::app) fn stop_migration() {
    let running = SWEEPING.with(|s| s.borrow_mut().running.take());
    drop(running);
}

impl Ctl {
    pub(in crate::app) fn wire_migration(me: &Rc<Self>, window: &crate::MainWindow) {
        let p = me.clone();
        window.global::<Upgraded>().on_close(move || {
            if let Some(w) = p.window() {
                w.global::<Upgraded>().set_shown(false);
            }
            p.tell_left();
        });
    }

    /// The sheet saying the sidecars' new format, once, where the support folder held something: the
    /// ledger records it only once a window shows it.
    pub fn tell_migration_if_due(&self) {
        let due = std::mem::take(&mut self.state.borrow_mut().nav.brought.tell);
        if let (true, Some(w)) = (due, self.window()) {
            w.global::<Upgraded>().set_shown(true);
            migration::told(&self.state.borrow().support);
        }
    }

    /// The files a sweep left as they were and has not said yet, said once in one notice by their
    /// reasons, never over the sheet: shown after it closes.
    pub(in crate::app) fn tell_left(&self) {
        let Some(w) = self.window() else { return };
        if w.global::<Upgraded>().get_shown() {
            return;
        }
        let support = self.state.borrow().support.clone();
        let Some(ledger) = migration::ledger::read(&support) else { return };
        let mut counts: std::collections::BTreeMap<Why, u64> = std::collections::BTreeMap::new();
        for left in ledger.left.values().filter(|l| !l.said) {
            *counts.entry(left.why).or_default() += 1;
        }
        let total: u64 = counts.values().sum();
        if total == 0 {
            return;
        }
        let lines: Vec<String> = counts.iter().map(|(why, n)| said(*why, *n)).collect();
        let title = trn("{n} photo keeps its earlier settings file", "{n} photos keep their earlier settings file", total);
        self.told(&title, &lines.join("\n"));
        let said = migration::ledger::update(&support, |l| l.left.values_mut().for_each(|left| left.said = true));
        if let Err(e) = said {
            crate::timing::line(&format!("migration: the ledger could not be written ({e})"));
        }
    }

    /// The sweep, when one is owed and none runs, over the indexes of `caches` or the launch's; the
    /// line follows it.
    pub fn start_migration(self: &Rc<Self>, caches: Option<&Path>) {
        let caches = SWEEPING.with(|s| {
            let mut s = s.borrow_mut();
            if let Some(c) = caches {
                s.caches = Some(c.to_path_buf());
            }
            let busy = s.running.as_ref().is_some_and(|(sweep, _)| sweep.running());
            s.caches.clone().filter(|_| !busy)
        });
        let Some(caches) = caches else { return };
        let place = {
            let s = self.state.borrow();
            Place { support: s.support.clone(), caches, computer: s.nav.computer.clone() }
        };
        let Some(sweep) = migration::start(place) else { return };
        let timer = Timer::default();
        let weak = Rc::downgrade(self);
        timer.start(slint::TimerMode::Repeated, WATCH, move || {
            if let Some(c) = weak.upgrade() {
                c.follow_migration();
            }
        });
        SWEEPING.with(|s| s.borrow_mut().running = Some((sweep, timer)));
    }

    /// The line while the sweep runs; at its end the line goes, and a notice says what was left.
    fn follow_migration(&self) {
        let Some(w) = self.window() else { return };
        let Some((running, done, total)) = SWEEPING.with(|s| {
            let s = s.borrow();
            let (sweep, _) = s.running.as_ref()?;
            let p = &sweep.progress;
            Some((sweep.running(), p.done.load(Ordering::Relaxed), p.total.load(Ordering::Relaxed)))
        }) else {
            return;
        };
        let lib = w.global::<Upgraded>();
        if running {
            let line = tr!("Bringing settings up to date: {done} of {total} folders", done = done, total = total.max(done));
            lib.set_line(line.into());
            lib.set_fraction(if total > 0 { done as f32 / total as f32 } else { -1.0 });
            return;
        }
        lib.set_line("".into());
        lib.set_fraction(-1.0);
        SWEEPING.with(|s| {
            if let Some((_, timer)) = s.borrow().running.as_ref() {
                timer.stop();
            }
        });
        self.tell_left();
    }
}

/// One reason the notice gives, with its count: each true of every file it counts.
fn said(why: Why, n: u64) -> String {
    match why {
        Why::Unwritable => trn(
            "{n} on a disk that refused the new file: read-only, full or locked. It shows its edits, and a change to it is refused until the disk takes it.",
            "{n} on a disk that refused the new file: read-only, full or locked. They show their edits, and a change to them is refused until the disk takes it.",
            n,
        ),
        Why::Unreadable => trn(
            "{n} whose settings file, or its folder's Prepare the Roll file, cannot be read. It is left exactly as it is.",
            "{n} whose settings file, or their folder's Prepare the Roll file, cannot be read. They are left exactly as they are.",
            n,
        ),
        Why::Away => trn("{n} whose original is not downloaded. It is converted once ONI opens it.", "{n} whose original is not downloaded. They are converted once ONI opens them.", n),
        Why::Original => trn(
            "{n} whose original could not be read to place its dust strokes or name its camera. It opens read-only.",
            "{n} whose original could not be read to place their dust strokes or name their camera. They open read-only.",
            n,
        ),
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_reason_says_only_what_its_files_do() {
        // A read writes nothing: a file on a disk that refused the new one opens with its edits.
        let refused = said(Why::Unwritable, 2);
        assert!(!refused.contains("open read-only") && refused.contains("refused until the disk takes it"), "{refused}");
        // Twin: a file held back for its original opens read-only, and says so.
        assert!(said(Why::Original, 2).contains("open read-only"));
    }
}
