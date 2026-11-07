//! Every key of the bar at one height, its glyph over its word, in both screens; a key that holds a state
//! pressed into the bar while it holds, as every other key of its kind, and plain again once it lets go.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::{Act, App, Bar, Library};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::ComponentHandle;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// `ButtonStyle.bar-key`: a keycap's 40 pt and a small gap.
const KEY: f32 = 48.0;

/// The one key of the bar so named, where the layout put it.
fn bar_key(w: &oni_app::MainWindow, label: &str) -> common::Placed {
    let keys: Vec<_> = common::placed(w, Role::Button, label).into_iter().filter(|k| k.y < w.global::<Bar>().get_height()).collect();
    assert_eq!(keys.len(), 1, "one {label:?} on the bar: {keys:?}");
    keys[0]
}

/// The pixel at `(x, y)` of the window as it stands, as its green level, the eye's brightness.
fn level(px: &(usize, Vec<slint::platform::software_renderer::Rgb565Pixel>), x: f32, y: f32) -> u16 {
    (px.1[y as usize * px.0 + x as usize].0 >> 5) & 0x3f
}

/// How many pixels of the box from `(x0, y0)` to `(x1, y1)` differ from the key's face beside its glyph.
fn inked(px: &(usize, Vec<slint::platform::software_renderer::Rgb565Pixel>), key: common::Placed, (x0, y0, x1, y1): (f32, f32, f32, f32)) -> usize {
    let face = px.1[(key.y + key.height / 2.0) as usize * px.0 + (key.x + 4.0) as usize].0;
    (y0 as usize..y1 as usize).flat_map(|y| (x0 as usize..x1 as usize).map(move |x| (x, y))).filter(|&(x, y)| px.1[y * px.0 + x].0 != face).count()
}

/// A worded key of the bar: its word written once, centred under the key's middle, and its glyph drawn above it.
fn stacked(rig: &common::Rig, w: &oni_app::MainWindow, label: &str, word: &str) {
    let key = bar_key(w, label);
    assert_eq!(key.height, KEY, "{label} as tall as every key of the bar");
    let words: Vec<_> = common::placed(w, Role::Text, word).into_iter().filter(|t| t.x >= key.x && t.x + t.width <= key.x + key.width && key.level_with(t)).collect();
    assert_eq!(words.len(), 1, "{word:?} written once on {label}: {words:?}");
    let (text, middle) = (words[0], key.middle());
    assert!((text.middle().0 - middle.0).abs() <= 1.0 && text.middle().1 > middle.1, "{label}'s word centred under its middle: {text:?} {key:?}");
    let px = rig.drawn();
    let glyph = inked(&px, key, (middle.0 - 8.0, key.y + 4.0, middle.0 + 8.0, text.y));
    assert!(glyph > 10, "{label}'s glyph drawn over its word: {glyph}");
}

pub(crate) fn every_bar_key_wears_its_glyph_over_its_word_and_a_key_on_is_pressed_in() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the bar's keys are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-toolbar-keys-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.5)] {
        common::graded_tiff(&folder.join(name), 120, 80, tone);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1728, 900));
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (lib, bar) = (w.global::<Library>(), w.global::<Bar>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let tile = |name: &str| oni_app::app::tiles(w).into_iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).index;
    lib.invoke_tile_clicked(tile("A"), false, false);
    rig.settle();

    // Every key of the bar one height, in both screens; the window's other keys are not the bar's form.
    let heights = || common::placed_all(w, Role::Button).into_iter().partition::<Vec<_>, _>(|k| k.y < bar.get_height());
    let (keys, others) = heights();
    assert!(keys.len() >= 9 && keys.iter().all(|k| k.height == KEY), "every key of the gallery's bar {KEY} pt tall: {keys:?}");
    assert!(others.iter().any(|k| k.height != KEY), "twin: the measure tells a key of another height");
    for (label, word) in [("EDIT", "EDIT"), ("Select All", "Select All"), ("PREPARE ROLL", "PREPARE ROLL"), ("Export\u{2026}", "Export\u{2026}"), ("Reset Photo", "Reset Photo"), ("Trash", "Trash"), ("Feedback", "Feedback"), ("Settings", "Settings"), ("ADD FOLDER\u{2026}", "ADD FOLDER\u{2026}")] {
        stacked(&rig, w, label, word);
    }
    // Twin: beside the glyph, within the key's padding, nothing is drawn but its face.
    let trash = bar_key(w, "Trash");
    let beside = inked(&rig.drawn(), trash, (trash.x + 3.0, trash.y + 8.0, trash.x + 6.0, trash.y + trash.height - 8.0));
    assert_eq!(beside, 0, "the glyph's measure sees the face alone beside it");

    lib.invoke_tile_opened(tile("A"));
    rig.until("A in the editor", Duration::from_secs(60), || !w.global::<App>().get_gallery() && ctl.state.borrow().renders() >= 1);
    rig.settle();
    let (keys, _) = heights();
    assert!(keys.len() >= 13 && keys.iter().all(|k| k.height == KEY), "every key of the editor's bar {KEY} pt tall: {keys:?}");
    for (label, word) in [("BACK", "BACK"), ("Undo", "Undo"), ("Crop", "Crop"), ("De-dust", "De-dust"), ("Compare", "Compare"), ("Copy", "Copy"), ("Paste to Roll", "Paste to Roll"), ("EXPORT", "EXPORT")] {
        stacked(&rig, w, label, word);
    }

    // A key that holds a state sinks below the bar's ground while it holds, plain above it otherwise,
    // the same for each, its width kept; a plain key beside it stays raised.
    let face = |label: &str| {
        let key = bar_key(w, label);
        let px = rig.drawn();
        let y = key.y + key.height / 2.0;
        (level(&px, key.x + 5.0, y), level(&px, key.x - 3.0, y), key.width)
    };
    for (label, act) in [("Crop", Act::Framing), ("De-dust", Act::Correction), ("Compare", Act::Compare), ("Select All", Act::SelectAll)] {
        let (off, ground, width) = face(label);
        assert!(off > ground, "{label} off: a plain key's face over the bar's ground: {off} {ground}");
        bar.invoke_act(act);
        rig.settle();
        let (on, ground, held) = face(label);
        println!("shot {}", rig.shot(&format!("toolbar-keys-{}", label.replace(' ', "-"))).display());
        assert!(on < ground && held == width, "{label} on: pressed in under the bar's ground, as wide: {on} {ground} {held} {width}");
        let (copy, ground, _) = face("Copy");
        assert!(copy > ground, "twin: Copy, which holds nothing, stays raised beside it: {copy} {ground}");
        bar.invoke_act(act);
        rig.settle();
        assert_eq!(face(label).0, off, "twin: {label} let go, plain again");
    }
    let _ = std::fs::remove_dir_all(&root);
}
