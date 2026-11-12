//! Projects as a hand meets them: two rolls' copies in one project, Add to Project said with Undo,
//! stocks off each roll, the rating filter, the empty tab, every drag a step, a Swift file read.

use oni_app::app::{self, Options, Shell};
use oni_app::{Library, Told};
use oni_core::{Archive, EditorStore};
use oni_library::{paths, project};
use oni_settings::Film;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

pub(crate) struct Hand {
    pub(crate) rig: common::Rig,
    pub(crate) shell: Shell,
    pub(crate) root: PathBuf,
}

impl Hand {
    /// A scans folder holding each of `files`, a TIFF of the tone given: one tone twice is one file
    /// twice, byte for byte.
    pub(crate) fn new(label: &str, files: &[(&str, f32)]) -> Hand {
        Hand::made(label, files, |_| {})
    }

    /// `new`, with `before` laying what the support folder holds when the app starts, on the root.
    pub(crate) fn made(label: &str, files: &[(&str, f32)], before: impl FnOnce(&std::path::Path)) -> Hand {
        let root = std::env::temp_dir().join(format!("oni-app-projects-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        // Each file dated a second after the one before, in the order given: the gallery lays the newest
        // first, and Windows' file clock, which steps by about 16 ms, gives files written together one time.
        let first = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        for (i, (file, tone)) in files.iter().enumerate() {
            let path = root.join("Scans").join(file);
            common::linear_tiff(&path, 48, 32, *tone);
            let dated = std::fs::File::options().write(true).open(&path).and_then(|f| f.set_modified(first + std::time::Duration::from_secs(i as u64)));
            dated.unwrap();
        }
        let root = PathBuf::from(paths::canonical(&root));
        before(&root);
        let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
        let rig = common::rig(shots);
        rig.window.set_size(slint::PhysicalSize::new(1280, 800));
        let gpu = oni_app::render::Device::open().expect("a GPU");
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
        let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
        shell.window.show().unwrap();
        Hand { rig, shell, root }
    }

    pub(crate) fn at(&self, file: &str) -> String {
        paths::join(&paths::canonical(&self.root.join("Scans")), file)
    }

    pub(crate) fn open(&self, folder: &str) {
        let folder = self.root.join("Scans").join(folder);
        self.shell.open_folder(&folder);
        let ctl = self.shell.ctl.clone();
        let want = paths::canonical(&folder);
        self.rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().opened() == Some(want.as_str()));
    }

    pub(crate) fn lib(&self) -> Library<'_> {
        self.shell.window.global::<Library>()
    }

    /// The tiles shown, by path in the grid's order.
    pub(crate) fn shown(&self) -> Vec<String> {
        let mut tiles = app::tiles(&self.shell.window);
        tiles.sort_by_key(|t| t.index);
        let order = self.shell.ctl.state.borrow().order().to_vec();
        tiles.into_iter().map(|t| order[t.index as usize].clone()).collect()
    }

    /// A click on the tile of `path`, past the double click; `shift` extends the selection.
    pub(crate) fn click(&self, path: &str, shift: bool) {
        std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
        self.rig.pump();
        let i = self.shell.ctl.state.borrow().order().iter().position(|p| p == path).unwrap_or_else(|| panic!("{path} shown"));
        self.lib().invoke_tile_clicked(i as i32, false, shift);
        self.rig.pump();
    }

    /// The right click on `path`'s tile, its line starting with `line`, then the submenu's `sub`.
    pub(crate) fn menu(&self, path: &str, line: &str, sub: Option<&str>) {
        let lib = self.lib();
        let i = self.shell.ctl.state.borrow().order().iter().position(|p| p == path).unwrap();
        let t = app::tiles(&self.shell.window).into_iter().find(|t| t.index as usize == i).unwrap();
        assert!(lib.invoke_context(t.x + t.width / 2.0, t.y + t.height / 2.0));
        let find = |lines: slint::ModelRc<oni_app::MenuLine>, text: &str| (0..lines.row_count()).position(|i| lines.row_data(i).is_some_and(|l| l.text.starts_with(text))).unwrap_or_else(|| panic!("{text} in the menu"));
        let opens = lib.invoke_menu_chosen(find(lib.get_menu_lines(), line) as i32);
        self.rig.pump();
        if let Some(sub) = sub {
            assert!(opens, "{line} opens its submenu");
            assert!(common::glyphed(lib.get_menu_sub_lines().iter()), "every line of {line}'s submenu wears its glyph");
            lib.invoke_menu_sub_chosen(find(lib.get_menu_sub_lines(), sub) as i32);
            self.rig.pump();
        }
    }

    pub(crate) fn told(&self) -> Option<(String, String, String)> {
        let t = self.shell.window.global::<Told>();
        t.get_shown().then(|| (t.get_title().to_string(), t.get_message().to_string(), t.get_action().to_string()))
    }

    pub(crate) fn members(&self) -> Vec<Vec<String>> {
        project::load(&self.root.join("Support")).projects.iter().map(|p| p.paths().into_iter().map(str::to_owned).collect()).collect()
    }

    /// A hand pressing on `path`'s tile, moving in steps to `to`, and letting go there: whether the
    /// drop's line showed as it let go.
    pub(crate) fn drag(&self, path: &str, to: (f32, f32)) -> bool {
        let lib = self.lib();
        let i = self.shell.ctl.state.borrow().order().iter().position(|p| p == path).unwrap();
        let t = app::tiles(&self.shell.window).into_iter().find(|t| t.index as usize == i).unwrap();
        let from = (lib.get_grid_x() + t.x + t.width / 2.0, lib.get_grid_y() + t.y - lib.get_scroll() + t.height / 2.0);
        let event = |e: WindowEvent| {
            self.rig.window.dispatch_event(e);
            self.rig.pump();
        };
        let (button, at) = (PointerEventButton::Left, |(x, y): (f32, f32)| LogicalPosition::new(x, y));
        event(WindowEvent::PointerMoved { position: at(from) });
        event(WindowEvent::PointerPressed { position: at(from), button });
        for k in 1..=8 {
            let f = k as f32 / 8.0;
            event(WindowEvent::PointerMoved { position: at((from.0 + (to.0 - from.0) * f, from.1 + (to.1 - from.1) * f)) });
        }
        let line = lib.get_drop_shown();
        event(WindowEvent::PointerReleased { position: at(to), button });
        line
    }

    /// Where a drop lands before `path`'s tile, a little left of its middle.
    pub(crate) fn before(&self, path: &str) -> (f32, f32) {
        let lib = self.lib();
        let i = self.shell.ctl.state.borrow().order().iter().position(|p| p == path).unwrap();
        let t = app::tiles(&self.shell.window).into_iter().find(|t| t.index as usize == i).unwrap();
        (lib.get_grid_x() + t.x + t.width / 2.0 - 30.0, lib.get_grid_y() + t.y - lib.get_scroll() + t.height / 2.0)
    }

    /// The projects' tree row titled `title` dragged onto the middle of the row titled `onto`.
    pub(crate) fn tree_drop(&self, title: &str, onto: &str) {
        let rows: Vec<oni_app::TreeRow> = self.lib().get_project_rows().iter().collect();
        let at = |t: &str| rows.iter().position(|r| r.title == t).unwrap_or_else(|| panic!("{t} has a row"));
        let middle = |i: usize| (rows[i].x + rows[i].width / 2.0, rows[i].y + rows[i].height / 2.0);
        let (from, to) = (middle(at(title)), middle(at(onto)));
        self.lib().invoke_project_row_dragged(at(title) as i32, from.0, from.1, to.0, to.1);
        self.rig.pump();
        self.lib().invoke_project_drag_ended(true);
        self.rig.pump();
    }

    pub(crate) fn project_titles(&self) -> Vec<String> {
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
/// Undo, each under its roll's heading; the stocks come from Roll 8's marker.
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

    // The Projects tab heads each copy with its roll, in the tree and in the grid; a roll's row
    // selects its photos. Its stocks come off Roll 8's marker alone.
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    hand.lib().invoke_project_row_folded(0);
    hand.rig.pump();
    let titles = hand.project_titles();
    assert_eq!(titles, ["Project 1", "Roll 10", "A.tif", "Roll 8", "A.tif", "B.tif"]);
    // Twin: the photos' rows alone name the two copies alike.
    assert_eq!(titles[2], titles[4]);
    let headings: Vec<(String, String)> = hand.lib().get_headings().iter().map(|h| (h.caption.into(), h.title.into())).collect();
    let heading = |roll: &str| ("Project 1".to_string(), roll.to_string());
    assert_eq!(headings, [heading("Roll 10"), heading("Roll 8")], "a band for each roll, under the project's name");
    assert_eq!(hand.shown(), [r10a.clone(), r8a.clone(), r8b.clone()]);
    let selected = |hand: &Hand| app::tiles(&hand.shell.window).iter().filter(|t| t.selected).count();
    hand.lib().invoke_project_row_clicked(3, false, false);
    hand.rig.pump();
    assert_eq!(selected(&hand), 2, "Roll 8's row selects its two photos");
    hand.lib().invoke_project_row_clicked(1, false, false);
    hand.rig.pump();
    assert_eq!(selected(&hand), 1, "twin: Roll 10's, its one");
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
    // File > New Project with nothing picked stays dark, and its act makes nothing.
    assert!(!hand.shell.window.global::<oni_app::Menus>().get_live().new_project);
    hand.shell.ctl.act(oni_app::Act::NewProject);
    hand.rig.pump();
    assert!(hand.members().is_empty(), "no project from no photo");
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
    assert_eq!(hand.shown(), std::slice::from_ref(&a), "Gold keeps A alone");
    lib.invoke_filter_picked(2);
    hand.rig.pump();
    assert!(hand.shown().is_empty());
    assert_eq!(lib.get_empty_title().as_str(), "No Silver Photos");
    // Twin: All shows the project whole again.
    lib.invoke_filter_picked(0);
    hand.rig.pump();
    assert_eq!(hand.shown(), [a, b]);
}

/// P holds A and B, Q holds C; X joins Q, then the grid drags A before C: ⌘Z takes back the drag,
/// then the add. A tree drop after Y's notice is a step of its own, which that notice never undoes.
pub(crate) fn every_drag_is_one_step_and_the_notice_undoes_its_own() {
    let tones = [("Roll/A.tif", 0.2), ("Roll/B.tif", 0.32), ("Roll/C.tif", 0.44), ("Roll/X.tif", 0.56), ("Roll/Y.tif", 0.68)];
    let hand = Hand::new("undo", &tones);
    let [a, b, c, x, y] = tones.map(|(f, _)| hand.at(f));
    hand.open("Roll");
    let ctl = hand.shell.ctl.clone();
    hand.click(&a, false);
    hand.click(&b, true);
    hand.menu(&a, "New Project from", None);
    hand.click(&c, false);
    hand.menu(&c, "New Project from", None);
    hand.click(&x, false);
    hand.menu(&x, "Add to Project", Some("Project 2"));
    assert_eq!(hand.members(), [vec![a.clone(), b.clone()], vec![c.clone(), x.clone()]]);

    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!(hand.shown(), [c.clone(), x.clone(), a.clone(), b.clone()], "Project 2, the newer, first");
    hand.drag(&a, hand.before(&c));
    let dragged = [vec![b.clone()], vec![a.clone(), c.clone(), x.clone()]];
    assert_eq!(hand.members(), dragged, "A leaves P for Q, before C");
    ctl.act(oni_app::Act::Undo);
    hand.rig.pump();
    // Twin: an undo of the add laid over the drag would leave A in no project.
    assert_eq!(hand.members(), [vec![a.clone(), b.clone()], vec![c.clone(), x.clone()]], "the drag taken back, A in P again");
    ctl.act(oni_app::Act::Undo);
    hand.rig.pump();
    assert_eq!(hand.members(), [vec![a.clone(), b.clone()], vec![c.clone()]], "then the add, X alone leaving");
    ctl.act(oni_app::Act::Redo);
    ctl.act(oni_app::Act::Redo);
    hand.rig.pump();
    assert_eq!(hand.members(), dragged);

    // Y joins Q, said with Undo; then B is dropped onto Q in the tree, a step of the same name.
    hand.lib().invoke_surface_picked(0);
    hand.rig.pump();
    hand.click(&y, false);
    hand.menu(&y, "Add to Project", Some("Project 2"));
    assert_eq!(hand.told().map(|t| t.2), Some("Undo".to_string()));
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    if !hand.lib().get_project_rows().iter().any(|r| r.title == "B.tif") {
        let at = hand.lib().get_project_rows().iter().position(|r| r.title == "Project 1").unwrap();
        hand.lib().invoke_project_row_folded(at as i32);
        hand.rig.pump();
    }
    hand.tree_drop("B.tif", "Project 2");
    let dropped = [vec![], vec![a.clone(), c.clone(), x.clone(), y.clone(), b.clone()]];
    assert_eq!(hand.members(), dropped, "B joins Q");
    assert!(hand.told().is_some(), "the notice still offers its Undo");
    hand.shell.window.global::<Told>().invoke_acted();
    hand.rig.pump();
    assert_eq!(hand.members(), dropped, "the drop is not the step the notice named");
    // Twin: the drop was a step, which the bar's key walks back.
    ctl.act(oni_app::Act::Undo);
    hand.rig.pump();
    assert_eq!(hand.members(), [vec![b], vec![a, c, x, y]]);
}

/// A Swift-era file names its photos by print: the app finds each at launch and writes nothing; the
/// first gesture writes this format, the former file kept beside it as it was.
pub(crate) fn a_swift_file_is_read_without_a_write_and_kept_at_the_first_change() {
    let tones = [("Roll/A.tif", 0.3), ("Roll/B.tif", 0.6)];
    let hand = Hand::made("swift", &tones, |root| {
        let prints: Vec<String> = tones.iter().map(|(f, _)| oni_library::relink::fingerprint(&root.join("Scans").join(f)).unwrap()).collect();
        let file = serde_json::json!({"version": 1, "projects": [{"id": oni_settings::Id::new().to_string(), "name": "Swift", "description": "", "createdAt": 8e8, "members": prints}]});
        std::fs::create_dir_all(root.join("Support")).unwrap();
        std::fs::write(project::file_path(&root.join("Support")), serde_json::to_vec(&file).unwrap()).unwrap();
    });
    let support = hand.root.join("Support");
    let original = std::fs::read(project::file_path(&support)).unwrap();
    let [a, b] = tones.map(|(f, _)| hand.at(f));
    hand.open("Roll");
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!(hand.shown(), [a.clone(), b.clone()], "each print found on its photo");
    assert_eq!(std::fs::read(project::file_path(&support)).unwrap(), original, "a read writes nothing");
    assert!(!project::former_path(&support, 1).exists());

    hand.click(&b, false);
    hand.menu(&b, "Remove from Project", Some("Swift"));
    assert_eq!(hand.shown(), [a]);
    assert_eq!(std::fs::read(project::former_path(&support, 1)).unwrap(), original, "the Swift file kept beside, as it was");
    let text = std::fs::read_to_string(project::file_path(&support)).unwrap();
    // Twin: the file written is this format's, no longer the one kept.
    assert!(text.contains(&format!("\"version\" : {}", project::FORMAT)) && text.contains("\"exactMembers\""), "{text}");
}
