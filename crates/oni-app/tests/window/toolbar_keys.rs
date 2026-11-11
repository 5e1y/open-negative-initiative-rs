//! Every key of the bar at one height, its glyph over its word, in both screens; a flat key shows no ground
//! at rest and one under the hand; a canvas tool's key, while it holds, shows no ground and no colour of its
//! own, and a tab of its tool's colour under it on the bar's bottom edge, as wide as the key, gone once it lets go.

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

/// Whether a pixel of the window's 16 bits shows the colour `rgb`, within the rounding the window keeps.
fn near(v: u16, rgb: u32) -> bool {
    let want = (((rgb >> 19) & 0x1f) << 11 | ((rgb >> 10) & 0x3f) << 5 | ((rgb >> 3) & 0x1f)) as u16;
    ((v >> 11) as i32 - (want >> 11) as i32).abs() <= 1 && (((v >> 5) & 0x3f) as i32 - ((want >> 5) & 0x3f) as i32).abs() <= 2 && ((v & 0x1f) as i32 - (want & 0x1f) as i32).abs() <= 1
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

pub(crate) fn every_bar_key_wears_its_glyph_over_its_word_and_a_key_on_lies_flat_and_lit() {
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
    for (label, word) in [("EDIT", "EDIT"), ("Sheet", "Sheet"), ("PREPARE ROLL", "PREPARE ROLL"), ("ADD FOLDER\u{2026}", "ADD FOLDER\u{2026}"), ("Reset", "Reset"), ("Trash", "Trash"), ("Feedback", "Feedback"), ("Settings", "Settings"), ("EXPORT", "EXPORT")] {
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
    // The faces' tabs stand as tall as the panel's, centred on the bar's line; twin: the keys beside them are taller.
    let tab = |label: &str| common::placed(w, Role::Tab, label)[0];
    let (face, panel, back) = (tab("Pro"), tab("Colour"), bar_key(w, "BACK"));
    assert_eq!(face.height, panel.height, "the faces' tabs as tall as the panel's: {face:?} {panel:?}");
    assert!((face.middle().1 - back.middle().1).abs() <= 0.5, "the faces' tabs centred on the bar's line: {face:?} {back:?}");
    assert!(back.height > panel.height, "twin: a key of the bar is taller than a tab: {back:?} {panel:?}");
    for (label, word) in [("BACK", "BACK"), ("Undo", "Undo"), ("Crop", "Crop"), ("De-dust", "De-dust"), ("Compare", "Compare"), ("Copy", "Copy"), ("Paste All", "Paste All"), ("EXPORT", "EXPORT")] {
        stacked(&rig, w, label, word);
    }

    // Inside a key's padding beside its glyph, at its top, its middle and its foot, with the bar's ground
    // over the key; and its width.
    let face = |label: &str| {
        let key = bar_key(w, label);
        let px = rig.drawn();
        let at = |y: f32| level(&px, key.x + 5.0, y);
        ([at(key.y + 3.0), at(key.y + key.height / 2.0), at(key.y + key.height - 4.0)], level(&px, key.x + 5.0, key.y - 3.0), key.width)
    };
    let hover = |label: &str| {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(bar_key(w, label).middle().0, bar_key(w, label).middle().1) });
        rig.settle();
    };
    let away = || {
        rig.window.dispatch_event(WindowEvent::PointerExited);
        rig.settle();
    };
    // Whether the box shows the colour, as the window's 16-bit pixels keep it.
    let shows = |px: &(usize, Vec<slint::platform::software_renderer::Rgb565Pixel>), r: common::Placed, rgb: u32| {
        (r.y as usize..(r.y + r.height) as usize).any(|y| (r.x as usize..(r.x + r.width) as usize).any(|x| near(px.1[y * px.0 + x].0, rgb)))
    };
    // Under a key, down to the bar's bottom edge: the room its tool's tab takes, its ends, and beside it.
    let below = |k: common::Placed| common::Placed { x: k.x + 2.0, y: k.y + k.height + 2.0, width: k.width - 4.0, height: 6.0 };
    let ends = |k: common::Placed| [common::Placed { x: k.x, y: k.y + k.height + 4.0, width: 2.0, height: 3.0 }, common::Placed { x: k.x + k.width - 2.0, y: k.y + k.height + 4.0, width: 2.0, height: 3.0 }];
    let beside = |k: common::Placed| common::Placed { x: k.x - 6.0, y: k.y + k.height + 4.0, width: 4.0, height: 3.0 };
    for (label, act, rgb) in [("Crop", Act::Framing, 0x3fae96), ("De-dust", Act::Correction, 0x9d7ff0), ("Compare", Act::Compare, 0xec6fa8), ("Sheet", Act::SelectAll, 0xe8873a)] {
        let (rest, ground, width) = face(label);
        assert_eq!(rest, [ground; 3], "{label} at rest: no ground of its own, the bar's around its glyph");
        let key = bar_key(w, label);
        assert!(!shows(&rig.drawn(), key, rgb) && !shows(&rig.drawn(), below(key), rgb), "twin: {label} at rest, neither its colour nor its tab");
        hover(label);
        let (under, ground, _) = face(label);
        println!("shot {}", rig.shot(&format!("toolbar-keys-{}-hover", label.replace(' ', "-"))).display());
        assert!(under[1] > ground, "twin: under the hand {label} shows a ground: {under:?} {ground}");
        away();
        bar.invoke_act(act);
        rig.settle();
        let (on, ground, held) = face(label);
        println!("shot {}", rig.shot(&format!("toolbar-keys-{}", label.replace(' ', "-"))).display());
        assert!(on == [ground; 3] && held == width, "{label} on: no ground of its own, as wide: {on:?} {ground} {held} {width}");
        let (px, key) = (rig.drawn(), bar_key(w, label));
        assert!(!shows(&px, key, rgb), "{label} on: the key itself takes no colour");
        assert!(shows(&px, below(key), rgb), "{label} on: its tab in its tool's colour under it");
        assert!(ends(key).iter().all(|e| shows(&px, *e, rgb)) && !shows(&px, beside(key), rgb), "{label}'s tab as wide as its key: {key:?}");
        let (copy, ground, _) = face("Copy");
        assert_eq!(copy, [ground; 3], "twin: Copy, which holds nothing, stays bare beside it: {copy:?} {ground}");
        bar.invoke_act(act);
        rig.settle();
        assert_eq!(face(label).0, rest, "twin: {label} let go, bare again");
        assert!(!shows(&rig.drawn(), below(bar_key(w, label)), rgb), "twin: {label} let go, its tab gone");
    }
    // Two tools at once: their rings nested in their keys' order, Crop's on the card's edge and Compare's a
    // stroke in; Compare's tab runs over Crop's ring onto its own, the inner over the outer.
    let (crop, compare) = (0x3fae96, 0xec6fa8);
    bar.invoke_act(Act::Framing);
    bar.invoke_act(Act::Compare);
    rig.settle();
    println!("shot {}", rig.shot("toolbar-keys-nested").display());
    let (px, back, tab) = (rig.drawn(), bar_key(w, "BACK"), bar_key(w, "Compare"));
    // The card's top row: the bar's bottom edge, a bar padding under its keys.
    let top = (back.y + back.height + 8.0) as usize;
    let row = |y: usize, rgb: u32| (0..px.0).filter(|&x| near(px.1[y * px.0 + x].0, rgb)).count();
    assert!(row(top, crop) > 500 && row(top + 1, compare) > 500, "Crop's ring on the card's edge, Compare's a stroke in: {} {}", row(top, crop), row(top + 1, compare));
    let over = (tab.x as usize + 2..(tab.x + tab.width) as usize - 2).all(|x| near(px.1[top * px.0 + x].0, compare));
    assert!(over, "Compare's tab over Crop's ring, onto its own: {tab:?}");
    assert!(near(px.1[top * px.0 + (tab.x - 20.0) as usize].0, crop), "twin: beside Compare's tab, Crop's ring");
    bar.invoke_act(Act::Compare);
    bar.invoke_act(Act::Framing);
    rig.settle();
    let after = rig.drawn();
    let left = (0..after.0).filter(|&x| near(after.1[top * after.0 + x].0, crop) || near(after.1[(top + 1) * after.0 + x].0, compare)).count();
    assert_eq!(left, 0, "twin: let go, no ring");
    let _ = std::fs::remove_dir_all(&root);
}
