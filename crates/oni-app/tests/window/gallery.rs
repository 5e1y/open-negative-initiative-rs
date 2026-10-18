//! The gallery on the headless window, driven by real pointer and key events: picks, opening, the
//! medal picker, the cell menu, projects, the archive that moves nothing, the ONI Trash, the sidebar.

use oni_app::app::{self, Options, Shell};
use oni_app::{App, Confirm, Library, Menus};
use oni_core::{Archive, EditorStore};
use oni_library::index::LibraryIndex;
use oni_settings::{Film, Medal};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model, SharedString};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::common;

struct Hand<'a> {
    rig: &'a common::Rig,
    w: &'a oni_app::MainWindow,
    popup: std::cell::Cell<bool>,
}

impl Hand<'_> {
    fn centre(&self, caption: &str) -> (f32, f32) {
        let lib = self.w.global::<Library>();
        let t = app::tiles(self.w).into_iter().find(|t| t.caption == caption).unwrap_or_else(|| panic!("{caption} is shown"));
        (lib.get_grid_x() + t.x + t.width / 2.0, lib.get_grid_y() + t.y - lib.get_scroll() + t.height / 2.0)
    }

    fn event(&self, e: WindowEvent) {
        self.rig.window.dispatch_event(e);
        self.rig.pump();
    }

    fn press_at(&self, (x, y): (f32, f32), button: PointerEventButton) {
        let position = LogicalPosition::new(x, y);
        self.event(WindowEvent::PointerMoved { position });
        self.event(WindowEvent::PointerPressed { position, button });
    }

    fn release_at(&self, (x, y): (f32, f32), button: PointerEventButton) {
        self.event(WindowEvent::PointerReleased { position: LogicalPosition::new(x, y), button });
    }

    fn click(&self, caption: &str, held: &[Key]) {
        let at = self.centre(caption);
        for k in held {
            self.event(WindowEvent::KeyPressed { text: (*k).into() });
        }
        self.press_at(at, PointerEventButton::Left);
        self.release_at(at, PointerEventButton::Left);
        for k in held.iter().rev() {
            self.event(WindowEvent::KeyReleased { text: (*k).into() });
        }
    }

    fn right_click(&self, caption: &str) {
        let at = self.centre(caption);
        self.press_at(at, PointerEventButton::Right);
        self.release_at(at, PointerEventButton::Right);
        self.popup.set(true);
    }

    /// A press carried in eight steps to `to`, as a hand drags.
    fn drag(&self, from: (f32, f32), to: (f32, f32)) {
        self.press_at(from, PointerEventButton::Left);
        for k in 1..=8 {
            let f = k as f32 / 8.0;
            let position = LogicalPosition::new(from.0 + (to.0 - from.0) * f, from.1 + (to.1 - from.1) * f);
            self.event(WindowEvent::PointerMoved { position });
        }
        self.release_at(to, PointerEventButton::Left);
    }

    /// Lets the system's double-click interval pass, so the next click is a first one.
    fn rest(&self) {
        let start = Instant::now();
        while start.elapsed() < oni_app::system::double_click() + Duration::from_millis(50) {
            self.rig.pump();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn selected(&self) -> Vec<String> {
        let mut tiles = app::tiles(self.w);
        tiles.sort_by_key(|t| t.index);
        tiles.into_iter().filter(|t| t.selected).map(|t| t.caption.to_string()).collect()
    }

    /// The sidebar's photo rows lit, in the tree's order.
    fn lit_rows(&self) -> Vec<String> {
        let rows = self.w.global::<Library>().get_rows();
        (0..rows.row_count()).filter_map(|i| rows.row_data(i)).filter(|r| r.kind == 2 && r.selected).map(|r| r.title.to_string()).collect()
    }

    fn shown(&self) -> Vec<String> {
        let mut tiles = app::tiles(self.w);
        tiles.sort_by_key(|t| t.index);
        tiles.into_iter().map(|t| t.caption.to_string()).collect()
    }

    fn menu(&self) -> Vec<String> {
        let lines = self.w.global::<Library>().get_menu_lines();
        (0..lines.row_count()).filter_map(|i| lines.row_data(i)).map(|l| l.text.to_string()).collect()
    }

    fn choose(&self, text: &str) -> bool {
        let at = self.menu().iter().position(|l| l == text).unwrap_or_else(|| panic!("{text} in {:?}", self.menu()));
        let kept = self.w.global::<Library>().invoke_menu_chosen(at as i32);
        self.rig.pump();
        if !kept && self.popup.replace(false) {
            // The popup closes as the line's own click would close it: a click on the inert status bar.
            let corner = (1270.0, 795.0);
            self.press_at(corner, PointerEventButton::Left);
            self.release_at(corner, PointerEventButton::Left);
        }
        kept
    }

    fn status(&self) -> String {
        self.w.global::<Library>().get_status().to_string()
    }
}

pub(crate) fn the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the gallery is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-gallery-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 1.0), ("B.tif", 0.8), ("C.tif", 0.6), ("D.tif", 0.4)] {
        common::linear_tiff(&folder.join(name), 96, 64, tone);
    }
    let canonical = |n: &str| oni_library::paths::canonical(&folder.join(n));
    oni_library::folders::set_roll(Some(&Film::new("Roll 7")), &oni_library::paths::canonical(&folder)).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: support.clone(), caches: caches.clone(), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    let hand = Hand { rig: &rig, w, popup: Default::default() };
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    assert_eq!(hand.shown(), ["A", "B", "C", "D"]);
    assert_eq!(hand.status(), "4 photos");
    let row = |p: &str| LibraryIndex::open(&LibraryIndex::default_path(&caches)).unwrap().all().into_iter().find(|e| e.path == p).unwrap();

    // `ListPicking`: a plain click selects, ⌘ adds, ⇧ runs from the anchor ⌘ moved.
    hand.click("A", &[]);
    assert_eq!(hand.selected(), ["A"]);
    assert_eq!(hand.status(), "A.tif", "one photo names itself");
    hand.click("C", &[Key::Control]);
    assert_eq!(hand.selected(), ["A", "C"]);
    assert_eq!(hand.status(), "2 selected");
    hand.click("D", &[Key::Shift]);
    assert_eq!(hand.selected(), ["A", "C", "D"], "⇧ runs from C, the anchor ⌘ set, and keeps A");
    let mut lit = hand.lit_rows();
    lit.sort();
    assert_eq!(lit, ["A", "C", "D"], "`TreeEngine.marked`: the sidebar lights the gallery's selection");
    hand.rest();
    hand.click("B", &[]);
    assert_eq!(hand.selected(), ["B"], "twin: a plain click replaces the selection");
    assert_eq!(hand.lit_rows(), ["B"], "and the sidebar follows it");
    hand.click("B", &[Key::Control]);
    assert!(hand.selected().is_empty());
    assert_eq!(hand.lit_rows(), ["B"], "twin: ⌘ empties the selection, the ring stays on B in the sidebar");
    hand.click("B", &[Key::Control]);
    // Under the last row the document has ended: no gesture there, the picks stay.
    let below = (lib.get_grid_x() + 8.0, lib.get_grid_y() + lib.get_content_height() - lib.get_scroll() + 8.0);
    hand.press_at(below, PointerEventButton::Left);
    hand.release_at(below, PointerEventButton::Left);
    assert_eq!(hand.selected(), ["B"], "a click under the rows picks no cell");

    // The second plain click on the same cell within the interval opens it; on another cell it does not.
    hand.rest();
    hand.click("A", &[]);
    hand.click("B", &[]);
    assert!(w.global::<App>().get_gallery(), "two cells clicked are no double click");
    hand.click("B", &[]);
    assert!(!w.global::<App>().get_gallery());
    assert_eq!(ctl.state.borrow().open_path(), Some(canonical("B.tif").as_str()));
    rig.chord(Key::Control, "g");
    assert!(w.global::<App>().get_gallery());
    hand.rest();

    // The arrows walk the cursor, ⇧ extends from the anchor, ⌘A takes the roll, Return opens.
    hand.click("A", &[]);
    let right: SharedString = Key::RightArrow.into();
    rig.key(&right);
    assert_eq!(hand.selected(), ["B"]);
    rig.chord(Key::Shift, &right);
    assert_eq!(hand.selected(), ["B", "C"], "⇧→ extends from B");
    rig.chord(Key::Control, "a");
    assert_eq!(hand.selected(), ["A", "B", "C", "D"]);
    let ret: SharedString = Key::Return.into();
    rig.key(&ret);
    assert!(!w.global::<App>().get_gallery());
    assert_eq!(ctl.state.borrow().open_path(), Some(canonical("C.tif").as_str()), "Return opens the cursor, C");
    assert_eq!(hand.lit_rows(), ["C"], "`designate`: the frame opened is the selection, ⌘A's four narrowed to it");
    rig.chord(Key::Control, "g");
    hand.rest();

    // The status bar's medal picker: one photo toggles its own medal off, several keep it.
    hand.click("A", &[]);
    assert!(lib.get_medal_shown());
    lib.invoke_medal_picked(3);
    rig.pump();
    assert_eq!(row(&canonical("A.tif")).label, Medal::Gold);
    assert_eq!(lib.get_medal_current(), 3);
    lib.invoke_medal_picked(3);
    rig.pump();
    assert_eq!(row(&canonical("A.tif")).label, Medal::None, "the lit medal tapped on one photo clears it");
    hand.click("B", &[Key::Control]);
    lib.invoke_medal_picked(2);
    rig.pump();
    lib.invoke_medal_picked(2);
    rig.pump();
    assert_eq!((row(&canonical("A.tif")).label, row(&canonical("B.tif")).label), (Medal::Silver, Medal::Silver), "twin: several keep it");

    // The cell menu counts its scope, and New Project from takes the two photos.
    hand.right_click("B");
    let menu = hand.menu();
    assert_eq!(menu[0], "Paste Settings (2 photos)");
    assert!(menu.contains(&"New Project from (2 photos)".to_string()) && menu.contains(&"Move 2 Photos to ONI Trash".to_string()), "{menu:?}");
    assert!(!menu.contains(&"Add to Project".to_string()), "no project yet, so no submenu");
    hand.choose("New Project from (2 photos)");
    let file = oni_library::project::load(&support);
    assert_eq!(file.projects.len(), 1);
    assert_eq!(file.projects[0].name, "Project 1");
    assert_eq!(file.projects[0].members.len(), 2);
    // A right click outside the selection acts on that cell alone.
    hand.right_click("D");
    assert!(hand.menu().contains(&"Add to Project".to_string()));
    assert!(hand.choose("Add to Project"), "a submenu keeps the menu open");
    assert_eq!(hand.menu(), ["Project 1"]);
    hand.choose("Project 1");
    assert_eq!(hand.selected(), ["D"], "a verb selects its scope");
    assert_eq!(oni_library::project::load(&support).projects[0].members.len(), 3);

    // The archive moves nothing by hand: a tile dragged onto another lands nowhere.
    let names = std::fs::read_dir(&folder).unwrap().count();
    hand.drag(hand.centre("D"), (hand.centre("A").0 - 30.0, hand.centre("A").1));
    assert_eq!(hand.shown(), ["A", "B", "C", "D"]);
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), names);
    assert!(Path::new(&canonical("D.tif")).exists());

    // A project rearranges its own photos: the same drag lands D before A.
    lib.invoke_surface_picked(1);
    rig.pump();
    assert_eq!(hand.shown(), ["A", "B", "D"]);
    let rows = lib.get_project_rows();
    assert_eq!(rows.row_count(), 1);
    assert_eq!(rows.row_data(0).unwrap().title, "Project 1");
    hand.drag(hand.centre("D"), (hand.centre("A").0 - 30.0, hand.centre("A").1));
    assert_eq!(hand.shown(), ["D", "A", "B"]);
    assert!(!lib.get_ghost_shown() && !lib.get_drop_shown(), "the ghost and the line leave with the drop");
    // Twin: a press that moves under the drag distance is a click, and reorders nothing.
    hand.rest();
    let a = hand.centre("A");
    hand.drag(a, (a.0 + 2.0, a.1));
    assert_eq!(hand.shown(), ["D", "A", "B"]);
    assert_eq!(hand.selected(), ["A"]);

    // The project's info: its name, its stocks counted by photo, a description kept on blur.
    lib.invoke_project_row_clicked(0);
    rig.pump();
    assert_eq!((lib.get_info_kind(), lib.get_info_title().as_str()), (2, "Project 1"));
    assert_eq!(lib.get_stocks().row_count(), 0, "no stock set yet");
    lib.invoke_description_committed("Coast".into());
    lib.invoke_title_committed("Coast trip".into());
    rig.pump();
    let file = oni_library::project::load(&support);
    assert_eq!((file.projects[0].name.as_str(), file.projects[0].description.as_str()), ("Coast trip", "Coast"));
    lib.invoke_title_committed("   ".into());
    assert_eq!(oni_library::project::load(&support).projects[0].name, "Coast trip", "a blank name is refused");

    // Delete Project asks, then the photos stay and the project goes.
    assert!(lib.invoke_project_row_menu(0));
    assert_eq!(hand.menu(), ["Delete Project"]);
    hand.choose("Delete Project");
    let confirm = w.global::<Confirm>();
    assert!(confirm.get_shown());
    assert_eq!(confirm.get_title().as_str(), "Delete \u{201c}Coast trip\u{201d}?");
    assert_eq!(confirm.get_key().as_str(), "DELETE PROJECT");
    rig.key(&ret);
    assert!(oni_library::project::load(&support).projects.is_empty());
    assert_eq!(lib.get_empty_title().as_str(), "No Photos in a Project");
    assert_eq!(lib.get_projects_empty().as_str(), "No Projects");
    lib.invoke_surface_picked(0);
    rig.pump();
    assert_eq!(hand.shown().len(), 4, "the photos are untouched");

    // The ONI Trash: ⌘⌫ marks C, the trash row counts it, a double click lays it as a grid.
    hand.rest();
    hand.click("C", &[]);
    let backspace: SharedString = Key::Backspace.into();
    rig.chord(Key::Control, &backspace);
    assert_eq!(hand.shown(), ["A", "B", "D"]);
    let trash = lib.get_trash_rows();
    assert_eq!((trash.row_data(0).unwrap().title.as_str(), trash.row_data(0).unwrap().count.as_str()), ("ONI Trash", "1"));
    // A real double click: two clicks, then the double click on the second.
    lib.invoke_trash_row_clicked(0);
    lib.invoke_trash_row_clicked(0);
    lib.invoke_trash_row_double_clicked(0);
    rig.pump();
    assert!(lib.get_trash_rows().row_data(0).unwrap().opened);
    assert_eq!(lib.get_trash_rows().row_count(), 2, "the double click leaves the trash unfolded on its folder");
    assert_eq!(hand.shown(), ["C"]);
    assert_eq!(hand.status(), "1 photo in the ONI Trash");
    hand.click("C", &[]);
    hand.click("C", &[]);
    assert!(w.global::<App>().get_gallery(), "a tile in the ONI Trash opens nothing");
    assert_eq!(hand.status(), "1 selected");
    let menus = w.global::<Menus>();
    assert!(menus.get_live().restore);
    assert_eq!(menus.get_titles().restore.as_str(), "Restore");
    hand.right_click("C");
    assert_eq!(hand.menu(), ["Restore", "Show in Finder"]);
    hand.choose("Restore");
    assert!(row(&canonical("C.tif")).trashed_at.is_none() && Path::new(&canonical("C.tif")).exists());
    assert_eq!(lib.get_empty_title().as_str(), "ONI Trash Is Empty");
    assert!(!menus.get_live().restore, "twin: nothing left to restore");
    lib.invoke_row_clicked(0);
    rig.pump();
    assert_eq!(hand.shown().len(), 4, "a row of the archive leaves the ONI Trash");

    // The roll's info writes the folder's marker: a camera body on Return, a date clamped.
    hand.rest();
    hand.click("A", &[]);
    assert_eq!((lib.get_info_kind(), lib.get_info_title().as_str()), (1, "Roll 7"));
    let marker = || oni_library::folders::roll(&oni_library::paths::canonical(&folder)).unwrap();
    lib.invoke_camera_body_committed("  Leica M6 ".into());
    rig.pump();
    assert_eq!(marker().camera_body.as_deref(), Some("Leica M6"));
    lib.invoke_shot_date_committed("4".into(), "31".into(), "2026".into());
    rig.pump();
    assert_eq!(marker().shot_date.map(oni_library::info::ymd), Some((2026, 4, 30)));
    assert!(lib.get_shot_date_set() && lib.get_day() == "30");
    lib.invoke_shot_date_committed("13".into(), "1".into(), "2026".into());
    rig.pump();
    assert_eq!(marker().shot_date.map(oni_library::info::ymd), Some((2026, 4, 30)), "twin: month 13 is refused");
    lib.invoke_shot_date_cleared();
    rig.pump();
    assert!(marker().shot_date.is_none() && !lib.get_shot_date_set());
    lib.invoke_title_committed("Roll 8".into());
    rig.pump();
    assert_eq!(marker().name, "Roll 8");
    rig.settle();
    println!("shot {}", rig.shot("gallery-organised").display());
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
