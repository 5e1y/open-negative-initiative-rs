//! What a drop, the picker, the command line and a launch open. A path handed over is looked at
//! off this thread first: one mistyped or on a disk away opens nothing, says so, and is never remembered.

use super::*;
use oni_library::navigator::Computer;
use oni_library::paths::{self, same};
use oni_library::presence::{self, Away, Presence};

/// What a path handed to the app opens, once looked at on the disk.
#[derive(Debug, Clone, PartialEq)]
pub enum Added {
    /// The folder to open, and the photo to open in the editor once it is read.
    Open { folder: String, opening: Option<String> },
    /// Nothing opens: the path is not there, or the disk refuses it.
    Refused { path: String, state: Presence },
    /// A file lying at the top of a whole disk or of the home, which is never read for one file.
    Wide { folder: String },
}

/// The first item's folder, the item itself when a folder, and the first photo among them. Reads the disk.
pub fn looked(items: &[PathBuf], computer: &Computer) -> Option<Added> {
    let first = items.first()?;
    let spelt = paths::normal(&first.to_string_lossy()).into_owned();
    presence::looked(&spelt);
    let folder = match std::fs::metadata(first) {
        Ok(m) if m.is_dir() => paths::canonical(first),
        Ok(_) => {
            let folder = paths::parent(&paths::canonical(first));
            if same(&folder, &computer.home) || computer.is_volume_root(&folder) || paths::parent(&folder) == folder {
                return Some(Added::Wide { folder });
            }
            folder
        }
        Err(e) => return Some(Added::Refused { state: presence::refused(&spelt, &e, computer), path: spelt }),
    };
    let opening = items.iter().find(|p| p.is_file() && oni_library::library::is_importable(p)).map(|p| paths::canonical(p));
    Some(Added::Open { folder, opening })
}

impl Ctl {
    /// `add(_:)`: the items looked at behind, then their folder opened, or why it is not said.
    /// Nothing is copied or moved.
    pub(in crate::app) fn add(self: &Rc<Self>, items: &[PathBuf]) {
        if items.is_empty() {
            return;
        }
        let (items, computer) = (items.to_vec(), self.state.borrow().nav.computer.clone());
        let _ = std::thread::Builder::new().name("adding".into()).spawn(move || {
            let added = looked(&items, &computer);
            with_shell(move |c| c.added(added));
        });
    }

    fn added(self: &Rc<Self>, added: Option<Added>) {
        match added {
            Some(Added::Open { folder, opening }) => {
                self.navigator_reveal(Path::new(&folder));
                self.open_spelt(folder);
                self.state.borrow_mut().opening = opening;
            }
            Some(Added::Refused { path, state: Presence::Offline(Away::Denied) }) => self.say(&tr!("Not opened: ONI has no access to {path}", path = path)),
            Some(Added::Refused { path, .. }) => self.say(&tr!("Not found: {path}", path = path)),
            Some(Added::Wide { folder }) => {
                self.say(&tr!("Not opened: {folder} is a whole disk or the home folder. Open the folder of scans inside it.", folder = folder))
            }
            None => {}
        }
    }

    /// `dropDestination`: files from the Finder are taken in the gallery alone, as ⌘O takes them;
    /// whether they were.
    pub fn dropped(self: &Rc<Self>, paths: &[PathBuf]) -> bool {
        // Under a sheet the drop would change the folder the sheet was asked about.
        let shown = self.window().is_some_and(|w| w.get_grid_shown() && !w.get_sheet_shown());
        if !shown || paths.is_empty() {
            return false;
        }
        self.add(paths);
        true
    }

    /// The launch's folder, Swift's `openedFolder ?? roots.current`: the one remembered opens
    /// whatever its state, so a disk away keeps its place and nothing replaces it.
    pub(in crate::app) fn open_remembered(self: &Rc<Self>) -> bool {
        let (remembered, support) = {
            let s = self.state.borrow();
            (s.preferences.string(preferences::OPENED_FOLDER).map(str::to_owned), s.support.clone())
        };
        let folder = remembered.or_else(|| oni_library::roots::Roots::load(&support).current().map(str::to_owned));
        match folder {
            Some(folder) => {
                self.open_spelt(paths::normal(&folder).into_owned());
                true
            }
            None => false,
        }
    }

    /// `open()`: the system's folder picker, then `add`; a cancel leaves everything as it was.
    pub(in crate::app) fn pick(self: &Rc<Self>) {
        // Taken out first: a modal panel runs a nested event loop, which may reach the state.
        let picker = std::mem::replace(&mut self.state.borrow_mut().picker, Box::new(|| None));
        let chosen = picker();
        self.state.borrow_mut().picker = picker;
        if let Some(folder) = chosen {
            self.add(&[folder]);
        } else if !system::HAS_PICKER {
            self.say(tr("This system has no folder picker: pass the folder on the command line"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_handed_over_opens_its_folder_or_nothing() {
        let root = std::env::temp_dir().join(format!("oni-app-adding-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for file in ["Home/Scans/A.tif", "Home/loose.tif", "Volumes/T7/top.tif"] {
            std::fs::create_dir_all(root.join(file).parent().unwrap()).unwrap();
            std::fs::write(root.join(file), b"a").unwrap();
        }
        let p = |rel: &str| paths::canonical(&root.join(rel));
        let computer = Computer { home: p("Home"), volumes: p("Volumes") };
        let scans = Added::Open { folder: p("Home/Scans"), opening: Some(p("Home/Scans/A.tif")) };
        assert_eq!(looked(&[root.join("Home/Scans/A.tif")], &computer), Some(scans));
        assert_eq!(looked(&[root.join("Home/Scans")], &computer), Some(Added::Open { folder: p("Home/Scans"), opening: None }));
        // A path mistyped: nothing, never the home it would have climbed to.
        let typo = paths::normal(&root.join("Home/Scnas").to_string_lossy()).into_owned();
        assert_eq!(looked(&[root.join("Home/Scnas")], &computer), Some(Added::Refused { path: typo, state: Presence::Missing }));
        // A file at the top of the home or of a volume never opens the whole of it.
        assert_eq!(looked(&[root.join("Home/loose.tif")], &computer), Some(Added::Wide { folder: p("Home") }));
        assert_eq!(looked(&[root.join("Volumes/T7/top.tif")], &computer), Some(Added::Wide { folder: p("Volumes/T7") }));
        // On a volume away: offline, and nothing opens either.
        let away = paths::normal(&root.join("Volumes/T8/Roll").to_string_lossy()).into_owned();
        assert_eq!(looked(&[root.join("Volumes/T8/Roll")], &computer), Some(Added::Refused { path: away, state: Presence::Offline(Away::Disconnected) }));
        let _ = std::fs::remove_dir_all(&root);
    }
}
