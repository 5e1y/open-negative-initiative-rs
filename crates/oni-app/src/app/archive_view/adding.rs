//! What a drop, the picker, the command line and a launch open, and what Add stars. A path handed over
//! is looked at off this thread first: one mistyped or on a disk away opens nothing, says so, and is
//! never remembered; a whole disk asks first, and items of other folders left out are counted.

use super::*;
use oni_library::navigator::Computer;
use oni_library::paths::{self, same};
use oni_library::presence::{self, Away, Presence};

/// What a path handed to the app opens, once looked at on the disk.
#[derive(Debug, Clone, PartialEq)]
pub enum Added {
    /// The folder to open, the photo to open in the editor once it is read, the items handed over
    /// that lie outside the folder, and whether it is a whole disk or the home.
    Open { folder: String, opening: Option<String>, left: usize, wide: bool },
    /// Nothing opens: the path is not there, or the disk refuses it.
    Refused { path: String, state: Presence },
    /// A file lying at the top of a whole disk or of the home, which is never read for one file.
    Wide { folder: String },
    /// Another app's package, a Photos or Photo Booth library: its photos are its app's to change.
    Package { package: String },
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
    if let Some(package) = oni_library::roots::package_holding(Path::new(&folder)) {
        return Some(Added::Package { package: paths::normal(&package.to_string_lossy()).into_owned() });
    }
    let inside = |p: &PathBuf| paths::holds(&folder, &paths::canonical(p));
    let opening = items.iter().find(|p| p.is_file() && oni_library::library::is_importable(p) && inside(p)).map(|p| paths::canonical(p));
    let left = items[1..].iter().filter(|p| !inside(p)).count();
    let wide = computer.is_wide(&folder);
    Some(Added::Open { folder, opening, left, wide })
}

impl Ctl {
    /// `add(_:)`: the items looked at behind, then their folder opened, or why it is not said.
    /// Nothing is copied or moved.
    pub(in crate::app) fn add(self: &Rc<Self>, items: &[PathBuf]) {
        self.add_as(items, false);
    }

    /// Add, as the welcome and the catch-up mean it: the folder starred, then opened.
    pub(in crate::app) fn add_starred(self: &Rc<Self>, items: &[PathBuf]) {
        self.add_as(items, true);
    }

    fn add_as(self: &Rc<Self>, items: &[PathBuf], star: bool) {
        if items.is_empty() {
            return;
        }
        let (items, computer) = (items.to_vec(), self.state.borrow().nav.computer.clone());
        let _ = std::thread::Builder::new().name("adding".into()).spawn(move || {
            let added = looked(&items, &computer);
            with_shell(move |c| c.added(added, star));
        });
    }

    fn added(self: &Rc<Self>, added: Option<Added>, star: bool) {
        match added {
            Some(Added::Open { folder, opening, left, wide }) => {
                let name = self.state.borrow().nav.computer.name(&folder);
                let open = move |c: &Rc<Ctl>| c.open_added(folder, opening, left, star);
                if !wide {
                    return open(self);
                }
                let message = tr("ONI reads every folder on it, which takes long on a full disk. A folder of scans inside it opens at once: pick one under it.");
                self.ask(&tr!("Show Every Photo on “{name}”?", name = name), message, tr("Show Every Photo"), false, Pending::Run(Box::new(open)));
            }
            Some(Added::Refused { path, state: Presence::Offline(Away::Denied) }) => self.told(tr("Open"), &tr!("Not opened: ONI has no access to {path}", path = path)),
            Some(Added::Refused { path, .. }) => self.told(tr("Open"), &tr!("Not found: {path}", path = path)),
            Some(Added::Wide { folder }) => {
                self.told(tr("Open"), &tr!("Not opened: {folder} is a whole disk or the home folder. Open the folder of scans inside it.", folder = folder))
            }
            Some(Added::Package { package }) => {
                let name = paths::name(&package);
                self.told(tr("Open"), &tr!("Not opened: “{name}” is another app's library, which keeps its photos inside. ONI changes nothing in it.", name = name))
            }
            None => {}
        }
    }

    /// The folder opened, starred first for Add, and the items handed over from other folders said.
    fn open_added(self: &Rc<Self>, folder: String, opening: Option<String>, left: usize, star: bool) {
        if star {
            self.star(&folder);
        }
        self.navigator_reveal(&folder);
        self.open_spelt(folder);
        self.state.borrow_mut().opening = opening;
        if left > 0 {
            self.told(tr("Open"), &trn("One item from another folder was left out: ONI opens one folder at a time.", "{n} items from other folders were left out: ONI opens one folder at a time.", left as u64));
        }
    }

    /// `dropDestination`: files from the Finder open as ⌘O opens them; from the editor the gallery
    /// comes back for them, and under a sheet or a roll being prepared the drop says why it waits.
    pub fn dropped(self: &Rc<Self>, paths: &[PathBuf]) -> bool {
        let Some(w) = self.window() else { return false };
        if paths.is_empty() {
            return false;
        }
        // Under a sheet the drop would change the folder the sheet was asked about.
        let busy = w.get_sheet_shown() || self.export.borrow().shown || self.state.borrow().prepare.shown;
        if busy {
            self.told(tr("Not Opened"), tr("Go back to the gallery to open these files."));
            return false;
        }
        if !w.get_grid_shown() {
            self.toggle_mode();
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

    /// `open()`: the system's folder picker, then the folder opened, starred too for Add; a cancel
    /// leaves everything as it was.
    pub(in crate::app) fn pick(self: &Rc<Self>, star: bool) {
        // Taken out first: a modal panel runs a nested event loop, which may reach the state.
        let picker = std::mem::replace(&mut self.state.borrow_mut().picker, Box::new(|| None));
        let chosen = picker();
        self.state.borrow_mut().picker = picker;
        if let Some(folder) = chosen {
            self.add_as(&[folder], star);
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
        let scans = Added::Open { folder: p("Home/Scans"), opening: Some(p("Home/Scans/A.tif")), left: 0, wide: false };
        assert_eq!(looked(&[root.join("Home/Scans/A.tif")], &computer), Some(scans));
        assert_eq!(looked(&[root.join("Home/Scans")], &computer), Some(Added::Open { folder: p("Home/Scans"), opening: None, left: 0, wide: false }));
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

    /// A drop from two folders opens the first one's and counts what it leaves out; the
    /// home or a whole disk dropped is marked wide, so it is asked about before any read.
    #[test]
    fn a_drop_from_two_folders_counts_what_it_leaves_and_a_whole_disk_is_marked() {
        let root = std::env::temp_dir().join(format!("oni-app-dropping-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for file in ["Home/A/1.tif", "Home/A/2.tif", "Home/B/3.tif", "Volumes/T7/Roll/4.tif"] {
            std::fs::create_dir_all(root.join(file).parent().unwrap()).unwrap();
            std::fs::write(root.join(file), b"a").unwrap();
        }
        let p = |rel: &str| paths::canonical(&root.join(rel));
        let computer = Computer { home: p("Home"), volumes: p("Volumes") };
        let left = |items: &[&str]| match looked(&items.iter().map(|i| root.join(i)).collect::<Vec<_>>(), &computer) {
            Some(Added::Open { left, .. }) => left,
            other => panic!("{other:?}"),
        };
        assert_eq!(left(&["Home/A/1.tif", "Home/B/3.tif", "Home/B"]), 2, "B's photo and B itself are left out");
        assert_eq!(left(&["Home/A/1.tif", "Home/A/2.tif"]), 0, "twin: one folder's files leave nothing out");
        let wide = |item: &str| matches!(looked(&[root.join(item)], &computer), Some(Added::Open { wide: true, .. }));
        assert!(wide("Home") && wide("Volumes/T7"), "the home and a disk dropped whole ask first");
        assert!(!wide("Volumes/T7/Roll"), "twin: a folder on the disk opens as it is");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A Photos library dropped, or a photo inside it, opens nothing: its photos are its app's.
    #[test]
    fn a_dropped_package_opens_nothing() {
        let root = std::env::temp_dir().join(format!("oni-app-package-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for file in ["Home/P.photoslibrary/originals/1.jpg", "Home/Scans/2.tif"] {
            std::fs::create_dir_all(root.join(file).parent().unwrap()).unwrap();
            std::fs::write(root.join(file), b"a").unwrap();
        }
        let p = |rel: &str| paths::canonical(&root.join(rel));
        let computer = Computer { home: p("Home"), volumes: p("Volumes") };
        let package = Added::Package { package: p("Home/P.photoslibrary") };
        assert_eq!(looked(&[root.join("Home/P.photoslibrary")], &computer), Some(package.clone()));
        assert_eq!(looked(&[root.join("Home/P.photoslibrary/originals/1.jpg")], &computer), Some(package));
        // Twin: a folder of scans beside it opens.
        assert!(matches!(looked(&[root.join("Home/Scans")], &computer), Some(Added::Open { .. })));
        let _ = std::fs::remove_dir_all(&root);
    }
}
