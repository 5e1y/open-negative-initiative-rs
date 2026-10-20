//! The library column follows the photo the editor opens: the folders holding it unfold, once per
//! photo so a fold made by hand stays, and its row comes into view; on the projects, its project.

use oni_app::app::{self, Options, Shell};
use oni_app::fields::Field;
use oni_app::{Library, Store as StoreGlobal, TreeRow};
use oni_core::{Archive, EditorStore};
use oni_library::project::{self, Project, ProjectsFile};
use oni_settings::Date;
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::common;

fn rows(lib: &Library) -> Vec<TreeRow> {
    lib.get_rows().iter().collect()
}

fn folder(lib: &Library, title: &str) -> (usize, TreeRow) {
    rows(lib).into_iter().enumerate().find(|(_, r)| r.kind != 2 && r.title == title).unwrap_or_else(|| panic!("{title} has a row"))
}

fn photo(lib: &Library, title: &str) -> Option<(usize, TreeRow)> {
    rows(lib).into_iter().enumerate().find(|(_, r)| r.kind == 2 && r.title == title)
}

/// Whether the row lies whole inside the column's view, `height` tall.
fn in_view(lib: &Library, height: f32, row: &TreeRow) -> bool {
    let top = -lib.get_tree_viewport_y();
    row.y >= top - 0.5 && row.y + row.height <= top + height + 0.5
}

/// Lets the system's double-click interval pass, so the next click is a first one.
fn rest(rig: &common::Rig) {
    let start = Instant::now();
    while start.elapsed() < oni_app::system::double_click() + Duration::from_millis(50) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The grid scrolled until `caption`'s tile shows, then opened as a double click opens it.
fn open_tile(rig: &common::Rig, w: &oni_app::MainWindow, caption: &str) {
    let lib = w.global::<Library>();
    lib.invoke_scrolled(-1.0e6);
    rig.pump();
    for _ in 0..60 {
        if let Some(t) = app::tiles(w).into_iter().find(|t| t.caption == caption) {
            lib.invoke_tile_opened(t.index);
            rig.pump();
            rig.drawn();
            rig.pump();
            return;
        }
        lib.invoke_scrolled(120.0);
        rig.pump();
    }
    panic!("{caption}'s tile never shows");
}

fn open_name(ctl: &std::rc::Rc<app::Ctl>) -> String {
    let s = ctl.state.borrow();
    Path::new(s.open_path().unwrap_or_default()).file_stem().unwrap().to_string_lossy().into_owned()
}

pub(crate) fn opening_a_photo_unfolds_its_folders_and_brings_its_row_into_view() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the column's reveal is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-sidebar-reveal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    // Tones apart, so no two files share a fingerprint.
    for i in 0..20 {
        common::plain_tiff(&scans.join("Roll A").join(format!("A{i:02}.tif")), 96, 64, 0.3 + i as f32 / 60.0);
        common::plain_tiff(&scans.join("Roll B").join(format!("B{i:02}.tif")), 96, 64, 0.65 + i as f32 / 100.0);
    }
    // Thirty one-photo projects around the one holding B05, so its row lands mid-list; a photo
    // sits in one project only, so the older half of them load empty.
    let fingerprint = |name: &str| {
        let path = oni_library::paths::canonical(&scans.join(name));
        oni_library::library::indexed(&path).expect("a readable TIFF").fingerprint
    };
    let at = |minutes: f64| Date::since_reference_date(8e8 + minutes * 60.0);
    let mut projects: Vec<Project> = (0..30).map(|i| Project::new(&format!("Crowd {i:02}"), at(i as f64), vec![fingerprint(&format!("Roll A/A{:02}.tif", i % 20))])).collect();
    projects.push(Project::new("Trip", at(15.5), vec![fingerprint("Roll B/B05.tif"), fingerprint("Roll B/B06.tif")]));
    project::save(&support, &ProjectsFile { version: 1, projects }).unwrap();

    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    shell.open_folder(&scans);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.drawn();

    // `openFolder`: the folder opened and every folder under it start unfolded.
    for title in ["Scans", "Roll A", "Roll B"] {
        assert!(folder(&lib, title).1.expanded, "{title} starts unfolded");
    }

    // Roll B folded by hand, then one of its photos opened from the grid: the roll unfolds and
    // the column brings the photo's row into view.
    lib.invoke_row_folded(folder(&lib, "Roll B").0 as i32);
    rig.pump();
    assert!(!folder(&lib, "Roll B").1.expanded && photo(&lib, "B18").is_none(), "a folded roll hides its photos");
    open_tile(&rig, w, "B18");
    rig.until("B18 open beside the column", Duration::from_secs(30), || open_name(&ctl) == "B18" && ctl.state.borrow().tree_height() > 0.0);
    let height = ctl.state.borrow().tree_height();
    rig.shot("sidebar-reveal-b18");
    assert!(folder(&lib, "Roll B").1.expanded, "opening B18 unfolds its roll");
    let (_, b18) = photo(&lib, "B18").expect("B18 has a row");
    assert!(lib.get_tree_viewport_y() < 0.0, "B18 sits below the column's first screen, so it scrolls");
    assert!(in_view(&lib, height, &b18), "B18's row is in view: {} at {} in {height}", b18.y, -lib.get_tree_viewport_y());
    let lower = TreeRow { y: b18.y + height, ..b18.clone() };
    assert!(!in_view(&lib, height, &lower), "twin: the same row a column lower is out of view");

    // A fold made by hand on the open photo's roll stays through an edit.
    lib.invoke_row_folded(folder(&lib, "Roll B").0 as i32);
    rig.pump();
    for _ in 0..2 {
        w.global::<StoreGlobal>().invoke_step(Field::Exposure.index(), 1);
        rig.pump();
    }
    assert!(!folder(&lib, "Roll B").1.expanded, "an edit keeps the hand's fold");

    // An arrow walks to the roll's next photo, which unfolds the roll again and shows its row.
    rig.key("\u{F703}");
    rig.pump();
    let walked = open_name(&ctl);
    assert!(walked.starts_with('B') && walked != "B18", "the arrow opens a neighbour in Roll B: {walked}");
    assert!(folder(&lib, "Roll B").1.expanded, "the walk unfolds the roll");
    let (_, row) = photo(&lib, &walked).expect("the walked photo has a row");
    assert!(in_view(&lib, height, &row), "{walked}'s row is in view");

    // `anchoredHere`: a row clicked opens its photo and leaves the column where the hand put it.
    let scrolled = lib.get_tree_viewport_y() + 40.0;
    lib.set_tree_viewport_y(scrolled);
    let (_, b16) = photo(&lib, "B16").expect("B16 has a row");
    assert!(in_view(&lib, height, &b16), "B16 is in view to be clicked");
    rest(&rig);
    lib.invoke_row_clicked(photo(&lib, "B16").unwrap().0 as i32, false, false);
    rig.pump();
    assert_eq!(open_name(&ctl), "B16");
    assert_eq!(lib.get_tree_viewport_y(), scrolled, "a click in the column never scrolls it");

    // The centre key unfolds the photo's roll again, a hand's fold included, and centres it.
    lib.invoke_row_folded(folder(&lib, "Roll B").0 as i32);
    rig.pump();
    assert!(!folder(&lib, "Roll B").1.expanded);
    lib.invoke_tree_centre();
    rig.pump();
    assert!(folder(&lib, "Roll B").1.expanded, "the centre key unfolds the roll");
    assert_ne!(lib.get_tree_viewport_y(), scrolled, "twin: the centre key moves the column");
    assert!(in_view(&lib, height, &photo(&lib, "B16").unwrap().1));

    // `openFromTree`: a folded roll's name opens its first photo and unfolds; its chevron only folds.
    lib.invoke_row_folded(folder(&lib, "Roll A").0 as i32);
    assert_eq!(open_name(&ctl), "B16", "twin: the chevron opens nothing");
    let still = lib.get_tree_viewport_y();
    rest(&rig);
    lib.invoke_row_clicked(folder(&lib, "Roll A").0 as i32, false, false);
    rig.pump();
    assert!(open_name(&ctl).starts_with('A'), "the roll's first photo opens: {}", open_name(&ctl));
    assert!(folder(&lib, "Roll A").1.expanded, "its roll unfolds");
    assert_eq!(lib.get_tree_viewport_y(), still, "and the column stays where the hand is");

    // On the projects, a photo opened rings under its project, which unfolds mid-list in view.
    lib.invoke_surface_picked(1);
    rig.drawn();
    rig.pump();
    let project_rows = || -> Vec<TreeRow> { lib.get_project_rows().iter().collect() };
    let trip = || project_rows().into_iter().position(|r| r.kind == 1 && r.title == "Trip").expect("Trip has a row");
    assert!(!project_rows()[trip()].expanded, "Trip starts folded");
    assert_eq!(lib.get_project_viewport_y(), 0.0);
    ctl.act("toggle-mode");
    open_tile(&rig, w, "B05");
    rig.until("B05 open", Duration::from_secs(30), || open_name(&ctl) == "B05");
    rig.drawn();
    rig.pump();
    let at = trip();
    assert!(project_rows()[at].expanded, "opening B05 unfolds Trip");
    // Beside a photo the projects' tree is laid as the archive's, a photo as a thumbnail by its stem.
    let b05 = project_rows()[at + 1].clone();
    assert!(b05.title == "B05" && b05.thumb && b05.selected, "B05 is ringed under Trip as a thumbnail");
    let offset = -lib.get_project_viewport_y();
    assert!(offset > 0.0, "Trip's photo sits below the list's first screen, so it scrolls");
    // Centred, the row's middle is half the list's height down from the top.
    let view = 2.0 * (b05.y + b05.height / 2.0 - offset);
    assert!((100.0..800.0).contains(&view), "B05's row centred in a list {view} tall");

    // A photo row clicked in the projects opens it and leaves the list where it is.
    rest(&rig);
    lib.invoke_project_row_clicked(at as i32 + 2, false, false);
    rig.pump();
    rig.drawn();
    assert_eq!(open_name(&ctl), "B06");
    assert_eq!(-lib.get_project_viewport_y(), offset, "a click in the projects never scrolls them");
    rig.settle();
    rig.shot("sidebar-reveal-projects");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
