//! The library column's last tree gestures, as `TreeEngine` gives them: the open row pinned, the projects
//! as names or thumbnails by each mode's key, a drag joining or reordering them, a click on no row.

use oni_app::app::{Options, Shell};
use oni_app::preferences::Preferences;
use oni_app::{Library, TreeRow};
use oni_core::{Archive, EditorStore};
use oni_library::project::{self, Project, ProjectsFile};
use oni_settings::Date;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::common;

/// Where the window lays the projects' list at 1280 × 800 beside a photo.
const LIST_TOP: f32 = 188.0;

fn project_rows(lib: &Library) -> Vec<TreeRow> {
    lib.get_project_rows().iter().collect()
}

fn at(lib: &Library, title: &str) -> usize {
    project_rows(lib).iter().position(|r| r.title == title).unwrap_or_else(|| panic!("{title} has a project row"))
}

/// Lets the system's double-click interval pass, so the next click is a first one.
fn rest(rig: &common::Rig) {
    let start = Instant::now();
    while start.elapsed() < oni_app::system::double_click() + Duration::from_millis(50) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A hand pressing at `from`, moving in steps to `to`, and letting go there, in window points.
fn drag(rig: &common::Rig, from: (f32, f32), to: (f32, f32)) {
    let point = |(x, y): (f32, f32)| LogicalPosition::new(x, y);
    let button = PointerEventButton::Left;
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: point(from) });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: point(from), button });
    for k in 1..=4 {
        let t = k as f32 / 4.0;
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: point((from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)) });
    }
    rig.pump();
}

fn release(rig: &common::Rig, to: (f32, f32)) {
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: LogicalPosition::new(to.0, to.1), button: PointerEventButton::Left });
    rig.pump();
}

pub(crate) fn the_trees_pin_their_open_row_lay_thumbnails_and_take_a_drop_as_swift() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the trees' last gestures are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-tree-leftovers-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for i in 0..24 {
        common::plain_tiff(&scans.join("Roll A").join(format!("A{i:02}.tif")), 96, 64, 0.3 + i as f32 / 90.0);
        common::plain_tiff(&scans.join("Roll B").join(format!("B{i:02}.tif")), 96, 64, 0.6 + i as f32 / 90.0);
    }
    let print = |name: &str| oni_library::library::indexed(&oni_library::paths::canonical(&scans.join(name))).expect("a readable TIFF").fingerprint;
    let when = |minutes: f64| Date::since_reference_date(8e8 + minutes * 60.0);
    let trip: Vec<String> = (0..20).map(|i| print(&format!("Roll B/B{i:02}.tif"))).collect();
    let walk: Vec<String> = (4..24).map(|i| print(&format!("Roll A/A{i:02}.tif"))).collect();
    project::save(&support, &ProjectsFile { version: 1, projects: vec![Project::new("Trip", when(2.0), trip.clone()), Project::new("Walk", when(1.0), walk.clone())] }).unwrap();
    let members = |name: &str| project::load(&support).projects.into_iter().find(|p| p.name == name).map(|p| p.members).unwrap_or_default();

    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: support.clone(), caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    let ds = w.global::<oni_app::DS>();
    let (h, left) = (ds.get_control_film_sidebar_row_height(), ds.get_control_card_inset());
    shell.open_folder(&scans);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("a photo rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    let open = || Path::new(ctl.state.borrow().open_path().unwrap_or_default()).file_stem().unwrap().to_string_lossy().into_owned();

    // Beside a photo the projects lay their photos two a line, captioned by their stems, pictured.
    lib.invoke_surface_picked(1);
    rig.pump();
    for name in ["Trip", "Walk"] {
        if !project_rows(&lib)[at(&lib, name)].expanded {
            lib.invoke_project_row_folded(at(&lib, name) as i32);
        }
    }
    let b00 = project_rows(&lib)[at(&lib, "B00")].clone();
    assert!(b00.thumb && b00.width < 150.0, "two thumbnails a line: {b00:?}");
    rig.until("the projects' thumbnails", Duration::from_secs(60), || project_rows(&lib).iter().filter(|r| r.thumb).all(|r| r.picture.size().width > 0));
    rig.shot("tree-leftovers-thumbnails");
    assert!(lib.get_tree_layout_help().starts_with("Two thumbnails a line"), "the key on the projects wears the layout");

    // The key cycles the editor's layout: one a line, then names; the gallery keeps its own, names.
    lib.invoke_tree_layout_next();
    rig.pump();
    assert!(project_rows(&lib)[at(&lib, "B00")].width > 2.0 * b00.width, "one thumbnail a line");
    lib.invoke_tree_layout_next();
    rig.pump();
    assert!(project_rows(&lib).iter().all(|r| !r.thumb) && project_rows(&lib).iter().any(|r| r.title == "B00.tif"), "names, in full");
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    assert!(project_rows(&lib).iter().all(|r| !r.thumb), "the gallery's projects start as names");
    lib.invoke_tree_layout_next();
    rig.pump();
    assert!(project_rows(&lib).iter().any(|r| r.thumb), "the gallery's key gives two a line");
    let prefs = Preferences::load(&support);
    assert_eq!((prefs.string("sidebarLayout.gallery"), prefs.string("sidebarLayout.editor")), (Some("pairs"), Some("list")));
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    assert!(project_rows(&lib).iter().all(|r| !r.thumb), "twin: beside a photo the editor's names stay");

    // `PinnedRow`: an open project whose photos pass under the top is held there until the next pushes it.
    let rows = project_rows(&lib);
    let walk_y = rows[at(&lib, "Walk")].y;
    let pin = || lib.get_project_pin();
    lib.set_project_viewport_y(-(rows[3].y + 1.0));
    rig.pump();
    assert!(pin().shown && pin().row.title == "Trip" && pin().push == 0.0, "Trip held over its photos: {:?}", pin());
    lib.set_project_viewport_y(-(walk_y - h / 2.0));
    rig.pump();
    assert!(pin().shown && pin().row.title == "Trip" && (pin().push + h / 2.0).abs() < 0.01, "Walk pushes Trip up: {:?}", pin());
    lib.set_project_viewport_y(-(walk_y + 3.0 * h));
    rig.pump();
    assert_eq!(pin().row.title, "Walk", "Walk takes its place");
    lib.set_project_viewport_y(0.0);
    rig.pump();
    assert!(!pin().shown, "twin: at the top, nothing is held");

    // A click on the pinned row clicks its project and centres it; its chevron folds it.
    lib.set_project_viewport_y(-(rows[3].y + 1.0));
    rig.pump();
    rest(&rig);
    let pinned_middle = (left + 120.0, LIST_TOP + h / 2.0);
    drag(&rig, pinned_middle, pinned_middle);
    release(&rig, pinned_middle);
    assert_eq!(open(), "B00", "beside a photo, Trip's first photo opens");
    assert!(project_rows(&lib)[at(&lib, "B00.tif")].selected, "and rings under Trip");
    assert_eq!(lib.get_project_viewport_y(), 0.0, "Trip brought back to the middle, held at the top");
    lib.set_project_viewport_y(-(rows[3].y + 1.0));
    rig.pump();
    rest(&rig);
    let chevron = (left + ds.get_space_sm() + ds.get_control_film_sidebar_chevron_zone_width() / 2.0, LIST_TOP + h / 2.0);
    drag(&rig, chevron, chevron);
    release(&rig, chevron);
    assert!(!project_rows(&lib)[at(&lib, "Trip")].expanded && pin().row.title != "Trip", "the chevron folds Trip, held no more");
    assert_eq!(open(), "B00", "twin: the fold opens nothing");

    // A photo dragged onto a project joins it and leaves its own; the ring and the chip show meanwhile.
    lib.set_project_viewport_y(0.0);
    rig.pump();
    let middle = |title: &str| {
        let r = project_rows(&lib)[at(&lib, title)].clone();
        (left + r.x + r.width / 2.0, LIST_TOP + r.y + r.height / 2.0)
    };
    let top_strip = |title: &str| {
        let r = project_rows(&lib)[at(&lib, title)].clone();
        (left + r.x + r.width / 2.0, LIST_TOP + r.y + 2.0)
    };
    rest(&rig);
    let (from, to) = (middle("A04.tif"), middle("Trip"));
    drag(&rig, from, to);
    let mark = lib.get_project_drag();
    let trip_row = project_rows(&lib)[at(&lib, "Trip")].clone();
    assert!(mark.shown && mark.ring && mark.chip == "1 item" && mark.mark_y == trip_row.y, "the ring round Trip: {mark:?}");
    rig.shot("tree-leftovers-drop");
    release(&rig, to);
    assert!(!lib.get_project_drag().shown, "the marks leave with the drop");
    assert_eq!(members("Trip").last(), Some(&print("Roll A/A04.tif")), "A04 joins Trip");
    assert!(!members("Walk").contains(&print("Roll A/A04.tif")) && members("Walk").len() == 19, "and leaves Walk");

    // Onto a row's top strip it lands before that row; onto a photo's middle it joins nothing.
    rest(&rig);
    let (from, to) = (middle("A06.tif"), top_strip("A05.tif"));
    drag(&rig, from, to);
    assert!(lib.get_project_drag().line, "a line before A05");
    release(&rig, to);
    assert_eq!(members("Walk")[..2], [print("Roll A/A06.tif"), print("Roll A/A05.tif")], "A06 lands before A05");
    rest(&rig);
    let before = members("Walk");
    let (from, to) = (middle("A07.tif"), middle("A05.tif"));
    drag(&rig, from, to);
    release(&rig, to);
    assert_eq!(members("Walk"), before, "twin: a photo dropped onto a photo joins nothing");

    // A project dragged onto another's top strip takes its place among the projects.
    rest(&rig);
    let (from, to) = (middle("Walk"), top_strip("Trip"));
    drag(&rig, from, to);
    release(&rig, to);
    let first = project_rows(&lib).into_iter().find(|r| r.kind == 1).unwrap();
    assert_eq!(first.title, "Walk", "Walk now heads the projects");

    // Twin: a press moved under the drag distance is a click, ringing its row, and moves nothing.
    rest(&rig);
    let before = (members("Trip"), members("Walk"));
    let (from, to) = (middle("A08.tif"), middle("A08.tif"));
    drag(&rig, from, (to.0 + 2.0, to.1));
    release(&rig, (to.0 + 2.0, to.1));
    assert!(!lib.get_project_drag().shown && (members("Trip"), members("Walk")) == before, "no drag");
    assert!(project_rows(&lib)[at(&lib, "A08.tif")].selected, "the press clicked A08");

    // `clickEmptyProjects`: a click on no row drops the ring and the selection.
    rest(&rig);
    let gap = (left + 120.0, LIST_TOP + 2.0);
    drag(&rig, gap, gap);
    release(&rig, gap);
    assert!(project_rows(&lib).iter().all(|r| !r.selected), "nothing is ringed: {:?}", project_rows(&lib).iter().filter(|r| r.selected).map(|r| r.title.to_string()).collect::<Vec<_>>());
    assert_eq!(open(), "A08", "the photo open stays open");

    // On the archive beside a photo, the open roll is held over its photos too.
    lib.invoke_surface_picked(0);
    rig.drawn();
    rig.pump();
    let rows: Vec<TreeRow> = lib.get_rows().iter().collect();
    let roll_a = rows.iter().position(|r| r.title == "Roll A").expect("Roll A has a row");
    lib.set_tree_viewport_y(0.0);
    rig.pump();
    assert!(!lib.get_tree_pin().shown, "at the top, nothing is held");
    lib.set_tree_viewport_y(-(rows[roll_a + 3].y + 1.0));
    rig.pump();
    assert!(lib.get_tree_pin().shown && lib.get_tree_pin().row.title == "Roll A", "{:?}", lib.get_tree_pin());
    let roll_b = rows.iter().position(|r| r.title == "Roll B").expect("Roll B has a row");
    lib.set_tree_viewport_y(-(rows[roll_b + 3].y + 1.0));
    rig.pump();
    assert_eq!(lib.get_tree_pin().row.title, "Roll B", "twin: over Roll B's photos, Roll B is held");
    rig.shot("tree-leftovers-archive-pin");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
