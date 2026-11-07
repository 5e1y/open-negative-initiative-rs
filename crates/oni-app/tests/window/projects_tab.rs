//! Projects as a hand meets them: two rolls' copies of one file in one project, Add to Project said
//! with Undo, stocks off each member's roll, the rating filter as in the archive, the empty tab.

use oni_app::app::{self, Options, Shell};
use oni_app::{Library, Told};
use oni_core::{Archive, EditorStore};
use oni_library::{paths, project};
use oni_settings::Film;
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

struct Hand {
    rig: common::Rig,
    shell: Shell,
    root: PathBuf,
}

impl Hand {
    /// A scans folder holding each of `files`, a TIFF of the tone given: one tone twice is one file
    /// twice, byte for byte.
    fn new(label: &str, files: &[(&str, f32)]) -> Hand {
        let root = std::env::temp_dir().join(format!("oni-app-projects-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (file, tone) in files {
            common::linear_tiff(&root.join("Scans").join(file), 48, 32, *tone);
        }
        let root = PathBuf::from(paths::canonical(&root));
        let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
        std::fs::create_dir_all(&shots).unwrap();
        let rig = common::rig(shots);
        rig.window.set_size(slint::PhysicalSize::new(1280, 800));
        let gpu = oni_app::render::Device::open().expect("a GPU");
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
        let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
        shell.window.show().unwrap();
        Hand { rig, shell, root }
    }

    fn at(&self, file: &str) -> String {
        paths::join(&paths::canonical(&self.root.join("Scans")), file)
    }

    fn open(&self, folder: &str) {
        let folder = self.root.join("Scans").join(folder);
        self.shell.open_folder(&folder);
        let ctl = self.shell.ctl.clone();
        let want = paths::canonical(&folder);
        self.rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().opened() == Some(want.as_str()));
    }

    fn lib(&self) -> Library<'_> {
        self.shell.window.global::<Library>()
    }

    /// The tiles shown, by path in the grid's order.
    fn shown(&self) -> Vec<String> {
        let mut tiles = app::tiles(&self.shell.window);
        tiles.sort_by_key(|t| t.index);
        let order = self.shell.ctl.state.borrow().order().to_vec();
        tiles.into_iter().map(|t| order[t.index as usize].clone()).collect()
    }

    /// A click on the tile of `path`, past the double click; `shift` extends the selection.
    fn click(&self, path: &str, shift: bool) {
        std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
        self.rig.pump();
        let i = self.shell.ctl.state.borrow().order().iter().position(|p| p == path).unwrap_or_else(|| panic!("{path} shown"));
        self.lib().invoke_tile_clicked(i as i32, false, shift);
        self.rig.pump();
    }

    /// The right click on `path`'s tile, its line starting with `line`, then the submenu's `sub`.
    fn menu(&self, path: &str, line: &str, sub: Option<&str>) {
        let lib = self.lib();
        let i = self.shell.ctl.state.borrow().order().iter().position(|p| p == path).unwrap();
        let t = app::tiles(&self.shell.window).into_iter().find(|t| t.index as usize == i).unwrap();
        assert!(lib.invoke_context(t.x + t.width / 2.0, t.y + t.height / 2.0));
        let find = |lines: slint::ModelRc<oni_app::MenuLine>, text: &str| (0..lines.row_count()).position(|i| lines.row_data(i).is_some_and(|l| l.text.starts_with(text))).unwrap_or_else(|| panic!("{text} in the menu"));
        let opens = lib.invoke_menu_chosen(find(lib.get_menu_lines(), line) as i32);
        self.rig.pump();
        if let Some(sub) = sub {
            assert!(opens, "{line} opens its submenu");
            lib.invoke_menu_sub_chosen(find(lib.get_menu_sub_lines(), sub) as i32);
            self.rig.pump();
        }
    }

    fn told(&self) -> Option<(String, String, String)> {
        let t = self.shell.window.global::<Told>();
        t.get_shown().then(|| (t.get_title().to_string(), t.get_message().to_string(), t.get_action().to_string()))
    }

    fn members(&self) -> Vec<Vec<String>> {
        project::load(&self.root.join("Support")).projects.into_iter().map(|p| p.members).collect()
    }

    fn project_titles(&self) -> Vec<String> {
        let rows = self.lib().get_project_rows();
        (0..rows.row_count()).filter_map(|i| rows.row_data(i)).map(|r| r.title.to_string()).collect()
    }
}

impl Drop for Hand {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Roll 10 holds a copy of Roll 8's A: Add to Project files Roll 8's A and B beside it, said with
/// Undo, each named with its roll where their names meet; the stocks come from Roll 8's marker.
pub(crate) fn two_rolls_copies_join_one_project_said_with_undo() {
    let hand = Hand::new("copies", &[("Roll 10/A.tif", 0.3), ("Roll 8/A.tif", 0.3), ("Roll 8/B.tif", 0.6)]);
    let mut gold = Film::new("Roll 8");
    gold.stock = Some("kodak-gold".into());
    oni_library::folders::set_roll(Some(&gold), &hand.at("Roll 8")).unwrap();
    let (r10a, r8a, r8b) = (hand.at("Roll 10/A.tif"), hand.at("Roll 8/A.tif"), hand.at("Roll 8/B.tif"));
    hand.open("");
    let ctl = hand.shell.ctl.clone();
    assert_eq!(ctl.state.borrow().fingerprint(&r10a).map(|f| f.split('~').next().unwrap().to_owned()), ctl.state.borrow().fingerprint(&r8a).map(|f| f.split('~').next().unwrap().to_owned()), "one file twice");

    hand.click(&r10a, false);
    hand.menu(&r10a, "New Project from", None);
    assert_eq!(hand.members(), [vec![r10a.clone()]]);
    hand.click(&r8a, false);
    hand.click(&r8b, true);
    hand.menu(&r8a, "Add to Project", Some("Project 1"));
    assert_eq!(hand.members(), [vec![r10a.clone(), r8a.clone(), r8b.clone()]], "the copy joins beside its twin");
    assert_eq!(hand.told(), Some(("Add to Project".into(), "2 photos added to “Project 1”.".into(), "Undo".into())));

    // Again: nothing joins, and the notice says why, with nothing to undo.
    hand.shell.window.global::<Told>().invoke_closed();
    hand.menu(&r8a, "Add to Project", Some("Project 1"));
    assert_eq!(hand.told(), Some(("Add to Project".into(), "The 2 photos are already in “Project 1”.".into(), String::new())));

    // The Projects tab names each copy with its roll; its stocks come off Roll 8's marker alone.
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    hand.lib().invoke_project_row_folded(0);
    hand.rig.pump();
    assert_eq!(hand.project_titles(), ["Project 1", "A.tif \u{b7} Roll 10", "A.tif \u{b7} Roll 8", "B.tif"]);
    hand.lib().invoke_project_row_clicked(0, false, false);
    hand.rig.pump();
    let stocks = |hand: &Hand| {
        let rows = hand.lib().get_stocks();
        (0..rows.row_count()).filter_map(|i| rows.row_data(i)).map(|s| (s.name.to_string(), s.count.to_string())).collect::<Vec<_>>()
    };
    assert_eq!(stocks(&hand), [("Kodak Gold".to_string(), "2".to_string())], "Roll 8's two photos; Roll 10 has no stock");
    // Roll 8 beyond the folder opened, its marker is read for the project all the same.
    hand.open("Roll 10");
    hand.lib().invoke_project_row_clicked(0, false, false);
    hand.rig.until("Roll 8's stock read beyond", Duration::from_secs(10), || stocks(&hand) == [("Kodak Gold".to_string(), "2".to_string())]);
    hand.rig.shot("projects-copies");

    // Undo walks the add back from the notice, as the bar's key would; Redo lays it again.
    hand.lib().invoke_surface_picked(0);
    hand.open("");
    hand.click(&r8b, false);
    hand.menu(&r8b, "Remove from Project", Some("Project 1"));
    assert_eq!(hand.members(), [vec![r10a.clone(), r8a.clone()]]);
    assert_eq!(hand.told().map(|t| t.1), Some("One photo removed from “Project 1”.".into()));
    hand.shell.window.global::<Told>().invoke_acted();
    hand.rig.pump();
    assert_eq!(hand.members(), [vec![r10a.clone(), r8a.clone(), r8b.clone()]], "Undo puts B back");
    ctl.act(oni_app::Act::Undo);
    hand.rig.pump();
    assert_eq!(hand.members(), [vec![r10a.clone()]], "the add undone, the project as it was");
    ctl.act(oni_app::Act::Redo);
    hand.rig.pump();
    // Twin: a redo lays the add again, so the step held both ends.
    assert_eq!(hand.members(), [vec![r10a, r8a, r8b]]);
}

/// The empty Projects tab makes a project; the rating filter keeps one medal there as in the archive.
pub(crate) fn the_empty_tab_makes_a_project_and_the_filter_acts_in_it() {
    let hand = Hand::new("filter", &[("Roll/A.tif", 0.3), ("Roll/B.tif", 0.6)]);
    let (a, b) = (hand.at("Roll/A.tif"), hand.at("Roll/B.tif"));
    hand.open("Roll");
    let lib = hand.lib();
    lib.invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!((lib.get_empty_title().as_str(), lib.get_empty_action().as_str()), ("No Photos in a Project", "New Project"));
    lib.invoke_empty_acted();
    hand.rig.pump();
    assert_eq!(hand.members(), [Vec::<String>::new()], "an empty project, named, to fill");
    let row = lib.get_project_rows().row_data(0).expect("its row");
    assert!(row.title == "Project 1" && row.renaming, "its name open to type");
    hand.rig.shot("projects-empty-made");

    lib.invoke_surface_picked(0);
    hand.rig.pump();
    hand.click(&a, false);
    hand.click(&b, true);
    hand.menu(&a, "Add to Project", Some("Project 1"));
    hand.click(&a, false);
    lib.invoke_medal_picked(1);
    hand.rig.pump();
    lib.invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!(hand.shown(), [a.clone(), b.clone()]);
    lib.invoke_filter_picked(1);
    hand.rig.pump();
    assert_eq!(hand.shown(), [a.clone()], "Gold keeps A alone");
    lib.invoke_filter_picked(2);
    hand.rig.pump();
    assert!(hand.shown().is_empty());
    assert_eq!(lib.get_empty_title().as_str(), "No Silver Photos");
    // Twin: All shows the project whole again.
    lib.invoke_filter_picked(0);
    hand.rig.pump();
    assert_eq!(hand.shown(), [a, b]);
}
