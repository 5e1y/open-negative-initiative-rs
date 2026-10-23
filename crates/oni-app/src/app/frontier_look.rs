//! The hand's Frontier look: a `.cube` it edits in any LUT tool on ONI's inversion, read again each
//! time the file is saved, until the hand returns to the measured table.

use super::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::time::SystemTime;

/// How often the file's date is looked at: a save shows within this.
const WATCH: Duration = Duration::from_secs(1);

struct Watched {
    path: PathBuf,
    modified: Option<SystemTime>,
    checked: Instant,
}

thread_local! {
    static WATCHED: RefCell<Option<Watched>> = const { RefCell::new(None) };
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl Ctl {
    /// The look the last session used, read again at launch; a file gone or broken falls back to
    /// the measured table and says so.
    pub(super) fn adopt_frontier_look(&self) {
        let kept = self.state.borrow().preferences.string(preferences::FRONTIER_LOOK).map(PathBuf::from);
        if let Some(path) = kept
            && let Err(e) = self.load_frontier_look(&path)
        {
            self.say(&tr!("The Frontier look could not be read again: {e}. The measured table is used.", e = e));
        }
    }

    /// Writes the look in use, else ONI's own as a start, where the hand chooses.
    pub(super) fn export_frontier_look(&self) {
        let cube = oni_pipeline::frontier_table::current().map(|(_, c)| (*c).clone()).unwrap_or_else(oni_pipeline::frontier_table::starting);
        let Some(path) = crate::system::save_file(tr("Export the Frontier look"), "ONI Frontier look.cube", &["cube"]) else { return };
        if let Err(e) = oni_library::write_atomically(&path, oni_pipeline::frontier_table::write(&cube).as_bytes()) {
            self.say(&tr!("The look could not be written: {e}", e = e));
        }
    }

    pub(super) fn choose_frontier_look(self: &Rc<Self>) {
        let Some(path) = crate::system::pick_file(tr("Use a Frontier look"), &["cube"]) else { return };
        match self.load_frontier_look(&path) {
            Ok(()) => self.remember(preferences::FRONTIER_LOOK, path.to_string_lossy().into_owned().into()),
            Err(e) => self.say(&tr!("This file cannot be the Frontier look: {e}", e = e)),
        }
    }

    pub(super) fn measured_frontier(&self) {
        oni_pipeline::frontier_table::set(None);
        WATCHED.with(|w| *w.borrow_mut() = None);
        self.remember(preferences::FRONTIER_LOOK, serde_json::Value::Null);
        self.frontier_changed();
    }

    /// From the pulse: a save of the file is read again; a half-written or broken one keeps the last.
    pub(super) fn watch_frontier_look(&self) {
        let due = WATCHED.with(|w| {
            let mut w = w.borrow_mut();
            let w = w.as_mut()?;
            if w.checked.elapsed() < WATCH {
                return None;
            }
            w.checked = Instant::now();
            let now = modified(&w.path);
            (now != w.modified).then(|| {
                w.modified = now;
                w.path.clone()
            })
        });
        if let Some(path) = due
            && let Err(e) = self.load_frontier_look(&path)
        {
            self.say(&tr!("The Frontier look's last save cannot be read: {e}", e = e));
        }
    }

    fn load_frontier_look(&self, path: &Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let cube = oni_pipeline::frontier_table::read(&text)?;
        oni_pipeline::frontier_table::set(Some(cube));
        WATCHED.with(|w| *w.borrow_mut() = Some(Watched { path: path.to_path_buf(), modified: modified(path), checked: Instant::now() }));
        self.frontier_changed();
        Ok(())
    }

    /// The open frame renders again on the new table, though none of its settings moved.
    fn frontier_changed(&self) {
        self.state.borrow_mut().requests.requested_view = None;
        crate::app::with_shell(|c| c.request_render());
        self.publish_settings();
    }

    /// What the sheet says of the look in use.
    pub(super) fn frontier_look_line(&self) -> String {
        WATCHED.with(|w| match w.borrow().as_ref() {
            Some(w) => tr!("From \u{201c}{file}\u{201d}, read again each time it is saved.", file = w.path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()),
            None => tr("The measured Frontier table.").into(),
        })
    }
}
