//! The right column's frame: one width from the gallery to the editor, so the card's edge holds,
//! and every block's title row leads with its fold's chevron at the column's inset however long its title.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::{Library, PanelState};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

const WIDTH: f32 = 1280.0;
/// Tall enough for the whole Colour tab, its last block's row in view.
const HEIGHT: u32 = 1400;
/// The editor's panel, `ShellStyle.panel-width`, and a block's inset in it, `SectionStyle.inset`.
const PANEL: f32 = 340.0;
const INSET: f32 = 16.0;
/// A title's start past the inset: the fold's chevron, `SectionStyle.fold`, and the row's step after it.
const FOLD: f32 = 10.0;
const STEP: f32 = 8.0;

/// The Colour tab's blocks, each with its fold key on its title's row.
const BLOCKS: [&str; 6] = ["Colour", "Lighting", "Effects", "Curves", "Colour Mixer", "Saturation Curve"];

/// A block's fold key, named by what it does: open, it hides the block; folded, it shows it.
fn fold_key(w: &oni_app::MainWindow, title: &str) -> common::Placed {
    let mut keys = common::placed(w, Role::Button, &format!("Hide {title}"));
    keys.extend(common::placed(w, Role::Button, &format!("Show {title}")));
    assert_eq!(keys.len(), 1, "one fold key for {title}: {keys:?}");
    keys[0]
}

/// Every key centred on the chevron's place at the column's inset, the same for each.
fn at_the_columns_inset(keys: &[common::Placed]) -> bool {
    keys.iter().all(|k| (k.x + k.width / 2.0 - (WIDTH - PANEL + INSET + FOLD / 2.0)).abs() < 0.5)
}

pub(crate) fn the_right_column_keeps_its_width_and_its_rows_keep_their_keys() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the panel's frame is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-panel-frame-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.6);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(WIDTH as u32, HEIGHT));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());

    // The gallery's column: its blocks start where the editor's do, a panel's width from the edge.
    lib.invoke_tile_clicked(0, false, false);
    rig.settle();
    assert_eq!(lib.get_info_kind(), 1, "the selection stands on a roll");
    let stock = common::the(w, Role::Text, "Film Stock");
    assert_eq!(stock.x, WIDTH - PANEL + INSET + FOLD + STEP, "the gallery's column is the editor's panel wide: {stock:?}");
    // Twin: a column 60 points narrower puts its blocks off the editor's edge.
    assert_ne!(stock.x, WIDTH - (PANEL - 60.0) + INSET + FOLD + STEP);
    rig.shot("panel-frame-gallery");

    // The editor's panel, by a double click, on the Colour tab: its blocks start at the same place.
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    w.global::<PanelState>().set_tab(2);
    w.global::<PanelState>().invoke_tab_picked(2);
    rig.settle();
    assert_eq!(w.get_panel_x(), WIDTH - PANEL, "the editor's panel");
    // The block's title, which the tab bar's word, in mid-bar, names too.
    let named = common::placed(w, Role::Text, "Colour");
    assert_eq!(named.len(), 2, "the tab and its block: {named:?}");
    let colour = named.into_iter().min_by(|a, b| a.x.total_cmp(&b.x)).unwrap();
    assert_eq!(colour.x, stock.x, "the editor's blocks start where the gallery's do: {colour:?}");

    // Every fold's chevron at the column's inset, before its title, the longest title's too.
    let keys: Vec<_> = BLOCKS.iter().map(|title| fold_key(w, title)).collect();
    assert!(at_the_columns_inset(&keys), "{:?}", BLOCKS.iter().zip(&keys).collect::<Vec<_>>());
    let title = common::the(w, Role::Text, "Saturation Curve");
    assert!(keys[5].x + keys[5].width / 2.0 < title.x && title.x + title.width <= WIDTH - INSET, "the longest title after its chevron, within the column: {title:?} {:?}", keys[5]);
    rig.shot("panel-frame-colour");
    // Twin: a key pushed three points along by its title is refused.
    let mut pushed = keys.clone();
    pushed[5].x += 3.0;
    assert!(!at_the_columns_inset(&pushed));
    let _ = std::fs::remove_dir_all(&root);
}
