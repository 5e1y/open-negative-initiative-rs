//! Rating where the eye is, by real events: the tile under the hand shows the podium's keys, a click lays one
//! on that photo alone; a medal laid lands, still under Reduce Motion or when a selection only shows it.

use i_slint_backend_testing::{AccessibleRole, ElementRoot};
use oni_app::app::{Options, Shell};
use oni_app::{Library, MainWindow, Motion};
use oni_core::{Archive, EditorStore};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, SharedString};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::common;

/// Slint's animations run this many times slower, so a read lands inside a 140 ms settle however
/// loaded the machine.
const SLOWED: u32 = 10;

/// A box of the window, left, top, right and bottom.
type Area = (f32, f32, f32, f32);

fn tile_area(w: &MainWindow, caption: &str) -> Area {
    let lib = w.global::<Library>();
    let t = oni_app::app::tiles(w).into_iter().find(|t| t.caption == caption).unwrap_or_else(|| panic!("{caption} is a tile"));
    let (x, y) = (lib.get_grid_x() + t.x, lib.get_grid_y() + t.y - lib.get_scroll());
    (x, y, x + t.width, y + t.height)
}

/// The status bar's band, under the grid.
fn bar_area(w: &MainWindow) -> Area {
    let lib = w.global::<Library>();
    (lib.get_grid_x(), lib.get_grid_y() + lib.get_grid_height(), lib.get_grid_x() + lib.get_grid_width(), f32::MAX)
}

fn inside((x0, y0, x1, y1): Area, (x, y): (f32, f32)) -> bool {
    x >= x0 && x < x1 && y >= y0 && y < y1
}

/// The medals drawn in a box, left to right: each coin's side as it stands now.
fn coins(w: &MainWindow, area: Area) -> Vec<f32> {
    let mut found: Vec<(f32, f32)> = w
        .root_element()
        .query_descendants()
        .match_descendants()
        .match_id("Medal::coin")
        .find_all()
        .into_iter()
        .filter_map(|e| {
            let (at, size) = (e.absolute_position(), e.size());
            inside(area, (at.x + size.width / 2.0, at.y + size.height / 2.0)).then_some((at.x, size.width))
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    // The search meets an element once per repeater it sits in.
    found.dedup_by(|a, b| a.0 == b.0);
    found.into_iter().map(|c| c.1).collect()
}

/// A key pressed as winit hands it over, the animations' clock read first.
fn lay(rig: &common::Rig, key: &str) {
    rig.pump();
    rig.key(key);
}

/// Long enough for a slowed landing to settle.
fn settled(rig: &common::Rig) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(SLOWED as u64 * 200) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(16));
    }
}

/// The medal keys in a box, left to right, by name.
fn keys(w: &MainWindow, area: Area) -> Vec<(String, (f32, f32))> {
    let mut found: Vec<(String, (f32, f32))> = ["Gold", "Silver", "Bronze"]
        .iter()
        .flat_map(|m| common::placed(w, AccessibleRole::Button, m).into_iter().map(move |p| (m.to_string(), p.middle())))
        .filter(|(_, at)| inside(area, *at))
        .collect();
    found.sort_by(|a, b| a.1.0.total_cmp(&b.1.0));
    found
}

fn medal_of(w: &MainWindow, caption: &str) -> i32 {
    oni_app::app::tiles(w).into_iter().find(|t| t.caption == caption).map_or(-1, |t| t.medal)
}

fn selected(w: &MainWindow) -> Vec<String> {
    oni_app::app::tiles(w).into_iter().filter(|t| t.selected).map(|t| t.caption.to_string()).collect()
}

/// The largest side a box's medals reach while the gesture's change lands, read after every pump: a
/// landing starts on the tick its change reaches Slint, so the read after that pump sees its first size.
fn widest(rig: &common::Rig, w: &MainWindow, area: Area, done: impl Fn() -> bool) -> f32 {
    let start = Instant::now();
    let mut widest = coins(w, area).into_iter().fold(0.0f32, f32::max);
    while start.elapsed() < Duration::from_millis(SLOWED as u64 * 60) || !done() {
        assert!(start.elapsed() < Duration::from_secs(30), "the medal never landed");
        rig.pump();
        widest = coins(w, area).into_iter().fold(widest, f32::max);
        std::thread::sleep(Duration::from_millis(1));
    }
    widest
}

pub(crate) fn the_tile_under_the_hand_takes_a_medal_and_a_medal_laid_lands() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the medal keys are not exercised"),
    };
    // SAFETY: set before the shell or any worker thread of this process starts.
    unsafe { std::env::set_var("SLINT_SLOW_ANIMATIONS", SLOWED.to_string()) };
    let root = std::env::temp_dir().join(format!("oni-app-medal-keys-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Scans");
    for (name, tone) in [("A.tif", 1.0), ("B.tif", 0.7), ("C.tif", 0.4)] {
        common::linear_tiff(&folder.join(name), 96, 64, tone);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let point = |(x, y): (f32, f32)| {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
        rig.pump();
    };
    let click = |at: (f32, f32)| {
        point(at);
        let position = LogicalPosition::new(at.0, at.1);
        rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
        rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
        rig.pump();
        rig.landed(&ctl);
    };
    let (a, b) = (tile_area(w, "A"), tile_area(w, "B"));
    let middle = |(x0, y0, x1, y1): Area| ((x0 + x1) / 2.0, (y0 + y1) / 2.0);

    // The tile under the hand shows gold, silver and bronze over its medal's corner; the others none.
    point(middle(a));
    let on_a = keys(w, a);
    let names: Vec<&str> = on_a.iter().map(|k| k.0.as_str()).collect();
    assert_eq!(names, ["Gold", "Silver", "Bronze"], "the podium under the hand, in its order");
    assert!(on_a.iter().all(|k| k.1.0 < middle(a).0 && k.1.1 > middle(a).1), "in the tile's lower left corner: {on_a:?}");
    // Twin: the tile beside it, not under the hand, shows none.
    assert!(keys(w, b).is_empty(), "the measure sees a tile with no keys");
    rig.settle();
    println!("shot {}", rig.shot("medal-keys-under-the-hand").display());

    // A click on a key lays its medal on that photo alone, leaving the selection; again, it comes off.
    click(on_a[1].1);
    assert_eq!(medal_of(w, "A"), 2, "the silver key lays silver on A");
    assert!(selected(w).is_empty(), "the key's click selects nothing");
    click(on_a[1].1);
    assert_eq!(medal_of(w, "A"), 0, "the lit key clicked takes its medal off");
    // Twin: a click on the photo beside the keys reaches the grid and picks it.
    click(middle(a));
    assert_eq!(selected(w), ["A"], "the measure sees a click on the photo pick it");

    // The key 1, the hand off the tiles: the medal lands larger and settles to its side.
    let below = (lib.get_grid_x() + 8.0, lib.get_grid_y() + lib.get_content_height() - lib.get_scroll() + 8.0);
    point(below);
    rig.settle();
    let side = w.global::<oni_app::DS>().get_control_icon_size();
    w.global::<Motion>().set_reduced(false);
    lay(&rig, "1");
    let landed = widest(&rig, w, a, || medal_of(w, "A") == 1);
    settled(&rig);
    let rest = coins(w, a);
    eprintln!("A's medal: {landed:.1} pt landing, {rest:?} at rest, its side {side}");
    assert!(landed >= side * 1.2, "the medal laid lands from above its side: {landed:.1} pt");
    assert_eq!(rest, [side], "and settles to its side");
    let bar = coins(w, bar_area(w));
    assert_eq!(bar.len(), 3, "the status bar's three medals: {bar:?}");

    // Twin: under Reduce Motion the medal stands still, gold turning silver.
    w.global::<Motion>().set_reduced(true);
    lay(&rig, "2");
    let still = widest(&rig, w, a, || medal_of(w, "A") == 2);
    assert!(still <= side + 0.5, "under Reduce Motion the medal stands still: {still:.1} pt");
    w.global::<Motion>().set_reduced(false);
    // Without it, a medal turning another lands as one laid afresh: silver turning bronze.
    lay(&rig, "3");
    let turned = widest(&rig, w, a, || medal_of(w, "A") == 3);
    settled(&rig);
    assert!(turned >= side * 1.2, "a medal turning another lands: {turned:.1} pt");

    // Twin: a selection moved to a photo wearing its medal shows it still in the bar.
    point(middle(b));
    click(keys(w, b)[1].1);
    assert_eq!((medal_of(w, "A"), medal_of(w, "B")), (3, 2), "silver on B, bronze on A");
    point(below);
    rig.settle();
    lay(&rig, &SharedString::from(Key::RightArrow));
    let bar_widest = widest(&rig, w, bar_area(w), || selected(w) == ["B"]);
    let picker = coins(w, bar_area(w));
    eprintln!("the bar's medals on B: {picker:?}, at most {bar_widest:.1} pt");
    assert!(bar_widest <= picker.iter().fold(0.0, |m: f32, c| m.max(*c)) + 0.5, "a selection's medal shows still: {bar_widest:.1} pt");
    // The key 1 on B: gold lands in the bar too.
    lay(&rig, "1");
    let bar_landed = widest(&rig, w, bar_area(w), || medal_of(w, "B") == 1);
    eprintln!("the bar's gold laid on B: {bar_landed:.1} pt");
    assert!(bar_landed > bar_widest + 1.0, "the bar's medal laid by the key lands: {bar_landed:.1} pt");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
