//! The library column: its tab bar, header and rows on one inset; a row clicked holds the keyboard until a
//! press or the editor hands it to the photos, the gallery listing no editor control; the select in words.

use i_slint_backend_testing::AccessibleRole;
use oni_app::app::{Options, Shell};
use oni_app::{FolderLine, Library, Navigator};
use oni_core::{Archive, EditorStore};
use oni_library::navigator::Computer;
use oni_library::paths::canonical;
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, Model, SharedString};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common::{self, Placed};

/// The column's edges as its tab bar draws them: the bar's hollow stands a tab's inset outside its tabs.
fn bar(w: &oni_app::MainWindow) -> (f32, f32) {
    let inset = w.global::<oni_app::DS>().get_space_xs();
    let (first, last) = (common::the(w, AccessibleRole::Tab, "Archive"), common::the(w, AccessibleRole::Tab, "Projects"));
    (first.x - inset, last.x + last.width + inset)
}

/// Between the bar's edges and on one of them, as a row's band, a thumbnail or a header's control.
fn lines_up(e: &Placed, (left, right): (f32, f32)) -> bool {
    let within = e.x >= left - 0.5 && e.x + e.width <= right + 0.5;
    within && ((e.x - left).abs() < 0.5 || (e.x + e.width - right).abs() < 0.5)
}

/// Every row and thumbnail the column shows.
fn listed(w: &oni_app::MainWindow) -> Vec<Placed> {
    let width = w.global::<oni_app::DS>().get_control_film_sidebar_width();
    common::placed_all(w, AccessibleRole::ListItem).into_iter().filter(|e| e.x + e.width <= width + 0.5).collect()
}

fn folders(w: &oni_app::MainWindow) -> Vec<FolderLine> {
    let rows = w.global::<Navigator>().get_rows();
    (0..rows.row_count()).filter_map(|i| rows.row_data(i)).collect()
}

/// The folders' rows as the arrows would leave them: titles, folds and the pick.
fn folds(w: &oni_app::MainWindow) -> Vec<(String, bool, bool)> {
    folders(w).iter().map(|l| (l.title.to_string(), l.unfolded, l.picked)).collect()
}

/// The photos the grid shows selected.
fn selected(lib: &Library) -> Vec<String> {
    lib.get_tiles().iter().filter(|t| t.selected).map(|t| t.caption.to_string()).collect()
}

pub(crate) fn the_column_lines_up_and_its_keyboard_shows_where_it_is() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the library column's edges and keys are not exercised"),
    };
    let made = std::env::temp_dir().join(format!("oni-app-column-edges-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&made);
    std::fs::create_dir_all(&made).unwrap();
    let root = PathBuf::from(canonical(&made));
    let (home, volumes, support, caches) = (root.join("Home"), root.join("Volumes"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A", 0.8), ("B", 0.6), ("C", 0.5), ("D", 0.4)] {
        common::linear_tiff(&home.join(format!("Scans/Roll 1/{name}.tif")), 64, 48, tone);
    }
    std::fs::create_dir_all(home.join("Scans/Roll 2")).unwrap();
    std::fs::create_dir_all(&volumes).unwrap();
    let p = |path: &Path| oni_library::paths::normal(&path.to_string_lossy()).into_owned();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let computer = Computer { home: p(&home), volumes: p(&volumes) };
    let options = Options { support, caches, picker: Box::new(|| None), computer, ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let lib = w.global::<Library>();
    shell.add(&home.join("Scans/Roll 1"));
    rig.until("Roll 1 read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().opened().is_some());
    rig.until("Roll 1 among the rows", Duration::from_secs(10), || folders(w).iter().any(|l| l.title == "Roll 1"));
    rig.settle();
    rig.shot("column-edges-gallery");

    // In the gallery the tab bar, the sections' headings and the rows' bands share the column's edges.
    let edges = bar(w);
    let heading = common::the(w, AccessibleRole::Button, "Hide Computer");
    assert!(lines_up(&heading, edges), "Computer's heading {heading:?} on the bar's edges {edges:?}");
    let rows = listed(w);
    let span = edges.1 - edges.0;
    assert!(!rows.is_empty() && rows.iter().all(|r| lines_up(r, edges) && (r.width - span).abs() < 0.5), "every band edge to edge: {rows:?} {edges:?}");
    // Twin: a band an `sm` wider on each side is off the bar's edges.
    assert!(!lines_up(&Placed { x: edges.0 - 8.0, width: span + 16.0, ..rows[0] }, edges));

    // A click on a folder's row gives the rows the keyboard, named for the ring the picked row wears.
    let roll1 = common::the(w, AccessibleRole::ListItem, "Roll 1").middle();
    common::click(&rig, w, roll1);
    rig.until("Roll 1 read again", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    assert_eq!(common::focused_label(w), "Folders", "the click gave the rows the keyboard");
    assert!(folders(w).iter().any(|l| l.title == "Roll 1" && l.picked), "the ring's row is the one clicked");
    rig.shot("column-edges-rows-keyed");
    // Twin: a press on the grid gives the photos the keyboard, whose arrow walks them.
    let right: SharedString = Key::RightArrow.into();
    let photo = common::placed_all(w, AccessibleRole::ListItem).into_iter().find(|e| e.x > edges.1).expect("a tile in the grid");
    common::click(&rig, w, photo.middle());
    assert_ne!(common::focused_label(w), "Folders", "a press on the grid takes the keyboard from the rows");
    let first = selected(&lib);
    rig.key(&right);
    assert!(!selected(&lib).is_empty() && selected(&lib) != first, "the arrow walks the photos: {first:?}");

    // The rows given the keyboard again, and the editor opened and left by the mode's key with no press
    // between, as from the menu: the way back gives the photos the arrows.
    common::click(&rig, w, common::the(w, AccessibleRole::ListItem, "Roll 1").middle());
    rig.until("Roll 1 read once more", Duration::from_secs(30), || ctl.state.borrow().read_done());
    assert_eq!(common::focused_label(w), "Folders", "the rows hold the keyboard as the editor opens");
    ctl.act(oni_app::Act::ToggleMode);
    rig.until("a photo rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    rig.shot("column-edges-beside-photo");

    // Beside a photo the tree's select and the rating filter span the column, and every row and
    // thumbnail meets its edges.
    let edges = bar(w);
    let select = common::the(w, AccessibleRole::Button, "Two per row");
    let filter = common::the(w, AccessibleRole::Button, "All");
    assert!((select.x - edges.0).abs() < 0.5 && (filter.x + filter.width - edges.1).abs() < 0.5 && select.level_with(&filter), "the select {select:?} and the filter {filter:?} across {edges:?}");
    let items = listed(w);
    assert!(items.iter().all(|e| lines_up(e, edges)), "rows and thumbnails on the edges: {items:?} {edges:?}");
    assert!(items.iter().any(|e| (e.x - edges.0).abs() < 0.5 && e.width < span / 2.0), "a thumbnail on the left edge: {items:?}");
    // Twin: beside a photo the editor's sliders are in the tree.
    assert!(!common::placed_all(w, AccessibleRole::Slider).is_empty(), "the panel's sliders are listed beside a photo");

    // The select's menu says each layout in words; a line picked lays the tree so and names it.
    common::click(&rig, w, select.middle());
    rig.settle();
    common::in_popup(w, AccessibleRole::Button, "Show the Selected Photo");
    common::click(&rig, w, common::in_popup(w, AccessibleRole::Button, "One per row").middle());
    rig.settle();
    assert_eq!(lib.get_tree_layout(), 2, "one per row picked");
    assert_eq!(common::placed(w, AccessibleRole::Button, "One per row").len(), 1, "the select names the layout picked");
    // Twin: the layout left is no longer named.
    assert!(common::placed(w, AccessibleRole::Button, "Two per row").is_empty());

    // Back in the gallery the editor's controls are out of the tree, and the arrows walk the photos.
    ctl.act(oni_app::Act::ToggleMode);
    rig.settle();
    let kept = folds(w);
    assert!(common::placed_all(w, AccessibleRole::Slider).is_empty(), "no slider of the editor is listed in the gallery");
    assert!(common::placed(w, AccessibleRole::Button, "One per row").is_empty(), "nor the tree's select");
    assert_ne!(common::focused_label(w), "Folders", "the screen changed, the photos hold the keyboard");
    let before = selected(&lib);
    rig.key(&right);
    assert!(!selected(&lib).is_empty() && selected(&lib) != before, "back from the editor the arrow walks the photos: {before:?}");
    assert_eq!(folds(w), kept, "and folds no folder");
    drop(shell);
    let _ = std::fs::remove_dir_all(&made);
}

/// An empty list says so where its first row would stand: at the list's top, on the rows' text inset,
/// its hint beneath, the projects' as the favourites'; the favourites' view is a key lit while on.
pub(crate) fn an_empty_list_speaks_at_its_top_on_the_rows_inset() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the column's empty lists are not exercised"),
    };
    let made = std::env::temp_dir().join(format!("oni-app-column-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&made);
    std::fs::create_dir_all(&made).unwrap();
    let root = PathBuf::from(canonical(&made));
    let (home, volumes) = (root.join("Home"), root.join("Volumes"));
    common::linear_tiff(&home.join("Scans/A.tif"), 64, 48, 0.5);
    std::fs::create_dir_all(&volumes).unwrap();
    let p = |path: &Path| oni_library::paths::normal(&path.to_string_lossy()).into_owned();
    let rig = common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let computer = Computer { home: p(&home), volumes: p(&volumes) };
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), computer, ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    shell.add(&home.join("Scans"));
    rig.until("Scans read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().opened().is_some());
    let text = |words: &str| common::the(w, AccessibleRole::Text, words);
    let margin = w.global::<oni_app::DS>().get_space_sm();
    // On the rows' text inset, a row at most under the header row its list hangs from, never mid column.
    let under = |e: &Placed, header: &str| {
        let (left, right) = bar(w);
        let row = common::placed(w, AccessibleRole::Button, header).into_iter().find(|k| k.x + k.width <= right + 0.5).expect("the header row's key");
        (e.x - (left + margin)).abs() < 0.5 && e.y > row.y + row.height && e.y < row.y + 2.0 * row.height
    };
    let at_top_on_inset = |e: &Placed| under(e, "All");

    w.global::<Library>().invoke_surface_picked(1);
    rig.settle();
    rig.shot("column-empty-projects");
    let (title, hint) = (text("No Projects"), text("Select photos in the Archive, then choose New Project."));
    assert!(at_top_on_inset(&title), "{title:?} at the list's top on the rows' inset {:?}", bar(w));
    assert!((hint.x - title.x).abs() < 0.5 && hint.y >= title.y + title.height, "the hint {hint:?} beneath {title:?}");
    // Twin: the grid's empty state, centred in the canvas, is neither.
    assert!(!at_top_on_inset(&text("No Photos in a Project")));

    // Each section folds under its heading, a key saying what it does and whether its rows show.
    let told = |label: &str| {
        let keys: Vec<_> = i_slint_backend_testing::ElementHandle::find_by_accessible_label(w, label).filter(|e| e.accessible_role() == Some(AccessibleRole::Button)).collect();
        assert_eq!(keys.len(), 1, "one key {label:?}");
        (keys[0].accessible_expanded(), keys[0].clone())
    };
    w.global::<Library>().invoke_surface_picked(0);
    rig.settle();
    let (open, key) = told("Hide Computer");
    assert_eq!(open, Some(true), "Computer's section stands open");
    key.invoke_accessible_default_action();
    rig.settle();
    assert_eq!(told("Show Computer").0, Some(false), "folded, its heading says it shows them again");
    drop(shell);
    let _ = std::fs::remove_dir_all(&made);
}
