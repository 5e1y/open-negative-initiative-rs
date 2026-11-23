//! A photo picked in the gallery is picked in the library column every time: the folders above it
//! unfold, a fold made by hand since included, and its row stands in view; on the projects, its project.

use oni_app::app::{self, Options, Shell};
use oni_app::{Library, TreeRow};
use oni_core::{Archive, EditorStore};
use oni_library::project::{self, Project, ProjectsFile};
use oni_settings::Date;
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::common;

/// Each roll's folder under the folder opened, and the letter its photos are named by.
const ROLLS: [(&str, &str); 4] = [("2023/Roll 1", "A"), ("2023/Roll 2", "B"), ("2024/Spring/Roll 3", "C"), ("2024/Summer/Deep/Roll 4", "D")];

fn rows(lib: &Library) -> Vec<TreeRow> {
    lib.get_rows().iter().collect()
}

fn folder(lib: &Library, title: &str) -> (usize, TreeRow) {
    rows(lib).into_iter().enumerate().find(|(_, r)| r.kind != 2 && r.title == title).unwrap_or_else(|| panic!("{title} has a row"))
}

/// Lets the system's double-click interval pass, so the next click is a first one.
fn rest(rig: &common::Rig) {
    let start = Instant::now();
    while start.elapsed() < oni_app::system::double_click() + Duration::from_millis(50) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The grid scrolled until `caption`'s tile shows, then clicked once.
fn pick(rig: &common::Rig, w: &oni_app::MainWindow, caption: &str) {
    let lib = w.global::<Library>();
    rest(rig);
    lib.invoke_scrolled(-1.0e6);
    rig.pump();
    for _ in 0..80 {
        if let Some(t) = app::tiles(w).into_iter().find(|t| t.caption == caption) {
            lib.invoke_tile_clicked(t.index, false, false);
            rig.pump();
            return;
        }
        lib.invoke_scrolled(120.0);
        rig.pump();
    }
    panic!("{caption}'s tile never shows");
}

/// The photo picked in the gallery shown beside the column, as Space shows it.
fn edit(rig: &common::Rig, ctl: &Rc<app::Ctl>) {
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    rig.drawn();
    rig.pump();
}

fn back(rig: &common::Rig, ctl: &Rc<app::Ctl>) {
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    rig.drawn();
}

/// What the column shows of `name`: its row picked, every folder above it unfolded, and the row
/// whole inside the view `height` tall; an error says which part is missing.
fn revealed(rows: &[TreeRow], top: f32, height: f32, above: &[&str], name: &str) -> Result<(), String> {
    for title in above {
        if !rows.iter().any(|r| r.kind != 2 && r.title == *title && r.expanded) {
            return Err(format!("{title} is not unfolded"));
        }
    }
    let row = rows.iter().find(|r| r.kind == 2 && r.title == name).ok_or_else(|| format!("{name} has no row"))?;
    if !row.selected {
        return Err(format!("{name}'s row is not picked"));
    }
    if row.y < top - 0.5 || row.y + row.height > top + height + 0.5 {
        return Err(format!("{name}'s row at {}..{} lies outside the view {top}..{}", row.y, row.y + row.height, top + height));
    }
    Ok(())
}

fn shown(lib: &Library, ctl: &Rc<app::Ctl>, above: &[&str], name: &str) -> Result<(), String> {
    let height = ctl.state.borrow().tree_height();
    assert!(height > 0.0, "the column has its size");
    revealed(&rows(lib), -lib.get_tree_viewport_y(), height, above, name)
}

fn open_name(ctl: &Rc<app::Ctl>) -> String {
    let s = ctl.state.borrow();
    Path::new(s.open_path().unwrap_or_default()).file_stem().unwrap().to_string_lossy().into_owned()
}

pub(crate) fn a_photo_picked_in_the_gallery_is_picked_unfolded_and_in_view_in_the_column() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the gallery's pick in the column is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-gallery-reveal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    // Tones apart, so no two files share a fingerprint; enough photos that a roll runs past the view.
    for (n, (path, letter)) in ROLLS.iter().enumerate() {
        for i in 0..14 {
            common::plain_tiff(&scans.join(path).join(format!("{letter}{i:02}.tif")), 96, 64, 0.2 + n as f32 * 0.2 + i as f32 / 100.0);
        }
    }
    let member = |name: &str| project::Member::new(Some(&oni_library::paths::canonical(&scans.join(name))), None);
    let at = |minutes: f64| Date::since_reference_date(8e8 + minutes * 60.0);
    let mut projects: Vec<Project> = (0..20).map(|i| Project::new(&format!("Crowd {i:02}"), at(i as f64), vec![member(&format!("2023/Roll 1/A{:02}.tif", i % 14))])).collect();
    projects.push(Project::new("Trip", at(10.5), vec![member("2024/Summer/Deep/Roll 4/D03.tif"), member("2024/Summer/Deep/Roll 4/D04.tif")]));
    project::save(&support, &ProjectsFile { version: project::FORMAT, projects }).unwrap();

    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    shell.open_folder(&scans);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.drawn();
    let deep = ["Scans", "2024", "Summer", "Deep", "Roll 4"];

    // D10 picked in the gallery deep in the tree, then shown beside the column.
    pick(&rig, w, "D10");
    edit(&rig, &ctl);
    assert_eq!(open_name(&ctl), "D10");
    shown(&lib, &ctl, &deep, "D10").unwrap();

    // The tree folded by hand above it hides it: the check's own sight fails there.
    lib.invoke_row_folded(folder(&lib, "2024").0 as i32);
    rig.pump();
    assert!(shown(&lib, &ctl, &deep, "D10").is_err(), "twin: a folded tree hides D10");
    common::scroll_tree(&lib, 0.0);
    rig.pump();

    // Back in the gallery, D10 picked again: the open photo, chosen anew, unfolds and shows.
    back(&rig, &ctl);
    pick(&rig, w, "D10");
    edit(&rig, &ctl);
    rig.shot("gallery-reveal-d10-again");
    shown(&lib, &ctl, &deep, "D10").unwrap();
    let height = ctl.state.borrow().tree_height();
    let (top, all) = (-lib.get_tree_viewport_y(), rows(&lib));
    assert!(top > 0.0, "D10 sits below the column's first screen, so it scrolls");
    assert!(revealed(&all, top + height, height, &deep, "D10").is_err(), "twin: a column lower, D10 is out of view");
    assert!(revealed(&all, top, height, &deep, "D09").is_err(), "twin: D09 is not picked");

    // Folded by hand, then a photo of the folded roll picked: it unfolds too.
    lib.invoke_row_folded(folder(&lib, "Roll 2").0 as i32);
    rig.pump();
    back(&rig, &ctl);
    pick(&rig, w, "B12");
    edit(&rig, &ctl);
    shown(&lib, &ctl, &["Scans", "2023", "Roll 2"], "B12").unwrap();

    // On the projects in the gallery, a photo picked rings under its project, which unfolds in view,
    // and again after the hand folds it.
    back(&rig, &ctl);
    lib.invoke_surface_picked(1);
    rig.drawn();
    rig.pump();
    let project_rows = || -> Vec<TreeRow> { lib.get_project_rows().iter().collect() };
    let trip = || project_rows().into_iter().position(|r| r.kind == 1 && r.title == "Trip").expect("Trip has a row");
    let in_view = |row: &TreeRow| {
        let (top, height) = (-lib.get_project_viewport_y(), ctl.state.borrow().project_tree_height());
        height > 0.0 && row.y >= top - 0.5 && row.y + row.height <= top + height + 0.5
    };
    let d03 = || project_rows().into_iter().find(|r| r.kind != 1 && r.kind != 3 && r.title.starts_with("D03"));
    for round in ["first", "after a fold"] {
        pick(&rig, w, "D03");
        rig.drawn();
        rig.pump();
        assert!(project_rows()[trip()].expanded, "Trip unfolds, {round}");
        let row = d03().unwrap_or_else(|| panic!("D03 has a row under Trip, {round}"));
        assert!(row.selected && in_view(&row), "D03 is picked and in view, {round}: {row:?}");
        lib.invoke_project_row_folded(trip() as i32);
        rig.pump();
        assert!(d03().is_none(), "twin: a folded Trip hides D03");
    }
    rig.settle();
    rig.shot("gallery-reveal-projects");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
