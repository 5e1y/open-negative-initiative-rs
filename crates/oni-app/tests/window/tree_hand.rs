//! The hand in the library column's trees, as `TreeEngine` reads it: ⌘ and ⇧ gather rows into the
//! selection, a right click opens Swift's menu on the pick or the row, a project's double click
//! types over its name, a click on no row drops the picks.

use oni_app::app::{Options, Shell};
use oni_app::{Library, Navigator, TreeRow};
use oni_core::{Archive, EditorStore};
use oni_library::project::{self, Project, ProjectsFile};
use oni_settings::Date;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::common;

fn rows(lib: &Library) -> Vec<TreeRow> {
    lib.get_rows().iter().collect()
}

fn row(lib: &Library, kind_photo: bool, title: &str) -> i32 {
    rows(lib).iter().position(|r| (r.kind == 2) == kind_photo && r.title == title).unwrap_or_else(|| panic!("{title} has a row")) as i32
}

/// The rows lit, by name in order.
fn marked(lib: &Library) -> Vec<String> {
    let mut lit: Vec<String> = rows(lib).into_iter().filter(|r| r.selected).map(|r| r.title.to_string()).collect();
    lit.sort();
    lit
}

fn lines(lib: &Library) -> Vec<String> {
    lib.get_menu_lines().iter().map(|l| l.text.to_string()).collect()
}

fn choose(lib: &Library, text: &str) {
    let at = lines(lib).iter().position(|l| l == text).unwrap_or_else(|| panic!("{text} in {:?}", lines(lib)));
    lib.invoke_menu_chosen(at as i32);
}

fn project_rows(lib: &Library) -> Vec<TreeRow> {
    lib.get_project_rows().iter().collect()
}

fn project_row(lib: &Library, title: &str) -> i32 {
    project_rows(lib).iter().position(|r| r.title == title).unwrap_or_else(|| panic!("{title} has a project row")) as i32
}

/// Where the window lays the projects' list at 1280 × 800, and a row's middle across it.
const LIST_TOP: f32 = 180.0;
const ACROSS: f32 = 120.0;

/// A press and release through the window, as a hand gives it, with a modifier held or none.
fn press(rig: &common::Rig, y: f32, button: PointerEventButton, held: Option<Key>) {
    let position = LogicalPosition::new(ACROSS, y);
    let held: Option<slint::SharedString> = held.map(Into::into);
    if let Some(k) = &held {
        rig.window.dispatch_event(WindowEvent::KeyPressed { text: k.clone() });
    }
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button });
    if let Some(k) = held {
        rig.window.dispatch_event(WindowEvent::KeyReleased { text: k });
    }
    rig.pump();
}

/// Lets the system's double-click interval pass, so the next click is a first one.
fn rest(rig: &common::Rig) {
    let start = Instant::now();
    while start.elapsed() < oni_app::system::double_click() + Duration::from_millis(50) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(crate) fn the_trees_pick_open_their_menus_and_rename_as_swift() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the trees' hand is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-tree-hand-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for i in 0..5 {
        common::plain_tiff(&scans.join("Roll A").join(format!("A{i:02}.tif")), 96, 64, 0.3 + i as f32 / 60.0);
        common::plain_tiff(&scans.join("Roll B").join(format!("B{i:02}.tif")), 96, 64, 0.65 + i as f32 / 100.0);
    }
    let fingerprint = |name: &str| {
        let path = oni_library::paths::canonical(&scans.join(name));
        oni_library::library::indexed(&path).expect("a readable TIFF").fingerprint
    };
    let at = |minutes: f64| Date::since_reference_date(8e8 + minutes * 60.0);
    let projects = vec![
        Project::new("Trip", at(2.0), vec![fingerprint("Roll B/B00.tif"), fingerprint("Roll B/B01.tif")]),
        Project::new("Walk", at(1.0), vec![fingerprint("Roll A/A04.tif")]),
    ];
    project::save(&support, &ProjectsFile { version: 1, projects }).unwrap();

    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: support.clone(), caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    shell.open_folder(&scans);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("a photo rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    let open = || Path::new(ctl.state.borrow().open_path().unwrap_or_default()).file_name().unwrap().to_string_lossy().into_owned();

    // `FilmRow.title`: a name row shows the whole file name; a thumbnail its stem.
    assert!(rows(&lib).iter().any(|r| r.thumb && r.title == "A01"), "a thumbnail is captioned by its stem");
    lib.invoke_tree_layout_next();
    lib.invoke_tree_layout_next();
    rig.pump();
    assert!(rows(&lib).iter().any(|r| !r.thumb && r.title == "A01.tif"), "twin: a name row keeps the extension");

    // A plain click opens; ⌘ gathers that row and the next; ⇧ adds the rows up to the anchor.
    rest(&rig);
    lib.invoke_row_clicked(row(&lib, true, "A01.tif"), false, false);
    rig.pump();
    assert_eq!(open(), "A01.tif");
    lib.invoke_row_clicked(row(&lib, true, "A03.tif"), true, false);
    rig.pump();
    assert_eq!(open(), "A01.tif", "a ⌘ click opens nothing");
    assert_eq!(marked(&lib), ["A01.tif", "A03.tif"], "the plain click's row joins the ⌘ pick");
    lib.invoke_row_clicked(row(&lib, true, "A04.tif"), false, true);
    rig.pump();
    assert_eq!(marked(&lib), ["A01.tif", "A03.tif", "A04.tif"], "⇧ adds the rows from the anchor");
    assert!(!marked(&lib).contains(&"A02.tif".to_string()), "twin: a row outside the range stays out");

    // A right click inside the pick acts on it whole; outside it, on its own row with Edit first.
    assert!(lib.invoke_row_menu(row(&lib, true, "A03.tif")));
    assert!(lines(&lib).contains(&"Paste Settings (3 photos)".to_string()) && !lines(&lib).contains(&"Edit".to_string()), "{:?}", lines(&lib));
    assert!(lib.invoke_row_menu(row(&lib, true, "A02.tif")));
    assert_eq!(lines(&lib)[0], "Edit", "twin: a row outside the pick is alone");

    // A plain click empties the pick; ⌘ on a folder brings every photo under it, and the anchor.
    rest(&rig);
    lib.invoke_row_clicked(row(&lib, true, "A02.tif"), false, false);
    rig.pump();
    assert_eq!(marked(&lib), ["A02.tif"], "a plain click leaves only its photo");
    lib.invoke_row_clicked(row(&lib, false, "Roll B"), true, false);
    rig.pump();
    assert_eq!(marked(&lib), ["A02.tif", "B00.tif", "B01.tif", "B02.tif", "B03.tif", "B04.tif", "Roll B"]);

    // A pick holding a folder and a photo shows in the Finder only; a folder alone takes the
    // navigator's folder menu, one menu for a folder wherever it is right-clicked.
    assert!(lib.invoke_row_menu(row(&lib, false, "Roll B")));
    assert_eq!(lines(&lib), ["Show in Finder"]);
    assert!(lib.invoke_row_menu(row(&lib, false, "Roll A")));
    assert_eq!(lines(&lib), ["Open", "Add to Favourites", "Rename…", "Make a Roll", "New Folder Inside", "Show in Finder"], "twin: a folder outside the pick");
    let ruled: Vec<bool> = lib.get_menu_lines().iter().map(|l| l.rule).collect();
    assert_eq!(ruled, [false, false, true, false, false, true], "dividers before Rename… and Show in Finder, as the navigator's");

    // Make a Roll writes the folder's marker, Back to a Folder takes it away.
    let roll_a = oni_library::paths::canonical(&scans.join("Roll A"));
    choose(&lib, "Make a Roll");
    rig.pump();
    assert!(oni_library::folders::roll(&roll_a).is_some(), "the folder is a roll");
    assert!(lib.invoke_row_menu(row(&lib, false, "Roll A")));
    assert_eq!(lines(&lib)[3], "Back to a Folder");
    choose(&lib, "Back to a Folder");
    rig.pump();
    assert!(oni_library::folders::roll(&roll_a).is_none(), "twin: the marker is gone");

    // New Folder Inside makes "New Folder", then "New Folder 2", each named in the tree it was asked from.
    for name in ["New Folder", "New Folder 2"] {
        assert!(lib.invoke_row_menu(row(&lib, false, "Roll A")));
        choose(&lib, "New Folder Inside");
        rig.until("the new folder named in the tree", Duration::from_secs(30), || rows(&lib).iter().any(|r| r.renaming && r.title == name));
        lib.invoke_row_rename_cancelled();
        rig.pump();
    }
    assert!(scans.join("Roll A/New Folder").is_dir() && scans.join("Roll A/New Folder 2").is_dir());
    assert!(w.global::<Navigator>().get_rows().iter().all(|r| !r.renaming), "twin: the tree's new folder is not named in the navigator");

    // Rename… types over the row; a click elsewhere commits what the field holds.
    assert!(lib.invoke_row_menu(row(&lib, false, "Roll A")));
    choose(&lib, "Rename…");
    rig.pump();
    let typed = rows(&lib).into_iter().find(|r| r.renaming).expect("a row typed over");
    assert_eq!(typed.title, "Roll A", "the field opens on the folder's name");
    lib.invoke_row_rename_cancelled();
    assert!(rows(&lib).iter().all(|r| !r.renaming) && scans.join("Roll A").is_dir(), "twin: Escape renames nothing");
    assert!(lib.invoke_row_menu(row(&lib, false, "Roll A")));
    choose(&lib, "Rename…");
    lib.invoke_row_rename_edited("Roll C".into());
    rest(&rig);
    lib.invoke_row_clicked(row(&lib, true, "B00.tif"), false, false);
    rig.until("Roll C read", Duration::from_secs(30), || rows(&lib).iter().any(|r| r.kind != 2 && r.title == "Roll C"));
    assert!(scans.join("Roll C/A00.tif").is_file() && !scans.join("Roll A").exists(), "the folder renamed on the disk");

    // A click on no row drops the picks and the ring; the photo stays open.
    assert!(!marked(&lib).is_empty(), "twin: the open photo is marked before");
    let opened = open();
    lib.invoke_tree_empty_clicked();
    rig.pump();
    assert!(marked(&lib).is_empty(), "nothing is marked: {:?}", marked(&lib));
    assert_eq!(open(), opened);

    // The projects: a double click types over a project's name, written trimmed to the file.
    lib.invoke_surface_picked(1);
    rig.pump();
    let trip = project_row(&lib, "Trip");
    rest(&rig);
    lib.invoke_project_row_clicked(trip, false, false);
    lib.invoke_project_row_clicked(trip, false, false);
    lib.invoke_project_row_double_clicked(trip);
    rig.pump();
    let typed = project_rows(&lib).into_iter().find(|r| r.renaming).expect("the project typed over");
    assert_eq!(typed.title, "Trip");
    lib.invoke_project_row_rename_committed(trip, "  Holiday ".into());
    rig.pump();
    let names = || project::load(&support).projects.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
    assert!(names().contains(&"Holiday".to_string()) && project_rows(&lib).iter().any(|r| r.title == "Holiday"), "{:?}", names());
    let walk = project_row(&lib, "Walk");
    rest(&rig);
    lib.invoke_project_row_clicked(walk, false, false);
    lib.invoke_project_row_double_clicked(walk);
    lib.invoke_project_row_rename_committed(walk, "   ".into());
    rig.pump();
    assert!(names().contains(&"Walk".to_string()), "twin: an empty name keeps the project's");

    // ⌘ on the projects' photos picks them; their menu acts on both, a project's is Delete Project.
    let holiday = project_row(&lib, "Holiday");
    if !project_rows(&lib)[holiday as usize].expanded {
        lib.invoke_project_row_folded(holiday);
    }
    rest(&rig);
    lib.invoke_project_row_clicked(project_row(&lib, "B00.tif"), false, false);
    lib.invoke_project_row_clicked(project_row(&lib, "B01.tif"), true, false);
    rig.pump();
    let picked: Vec<String> = project_rows(&lib).into_iter().filter(|r| r.selected).map(|r| r.title.to_string()).collect();
    assert_eq!(picked, ["B00.tif", "B01.tif"]);
    assert!(lib.invoke_project_row_menu(project_row(&lib, "B01.tif")));
    assert!(lines(&lib).contains(&"Paste Settings (2 photos)".to_string()), "{:?}", lines(&lib));
    assert!(lib.invoke_project_row_menu(project_row(&lib, "Holiday")));
    assert_eq!(lines(&lib), ["Delete Project"]);
    rest(&rig);
    lib.invoke_project_row_clicked(project_row(&lib, "Holiday"), false, false);
    lib.invoke_project_row_clicked(project_row(&lib, "Walk"), true, false);
    assert!(!lib.invoke_project_row_menu(project_row(&lib, "Walk")), "twin: two projects open no menu");

    // Through the pointer: a plain click drops the pick, ⌘ held adds a row, a right click opens its menu.
    let ds = w.global::<oni_app::DS>();
    let (height, margin) = (ds.get_control_film_sidebar_row_height(), ds.get_space_sm());
    let y = |title: &str| LIST_TOP + margin + (project_row(&lib, title) as f32 + 0.5) * height;
    let lit = || project_rows(&lib).into_iter().filter(|r| r.selected).map(|r| r.title.to_string()).collect::<Vec<_>>();
    rest(&rig);
    press(&rig, y("B00.tif"), PointerEventButton::Left, None);
    assert_eq!(lit(), ["B00.tif"], "a plain click leaves its row alone");
    press(&rig, y("B01.tif"), PointerEventButton::Left, Some(Key::Control));
    assert_eq!(lit(), ["B00.tif", "B01.tif"], "twin: ⌘ held adds the row");
    lib.set_menu_lines(Default::default());
    press(&rig, y("B01.tif"), PointerEventButton::Right, None);
    assert!(lines(&lib).contains(&"Paste Settings (2 photos)".to_string()), "{:?}", lines(&lib));
    rig.shot("tree-hand-projects");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
