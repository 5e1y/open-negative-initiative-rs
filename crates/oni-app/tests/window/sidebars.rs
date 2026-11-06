//! The sidebars' keys where Swift lays them, read off the pointer's notes: beside a photo the
//! column's keys sit on one mini plate left of the folder's pill, a filling plate spans its block with
//! its words evenly apart, and a black and white photo has no Color block.

use oni_app::app::{Options, Shell};
use oni_app::{Framing, Library, Navigator, PanelState, Tip};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::platform::software_renderer::Rgb565Pixel;
use slint::{ComponentHandle, LogicalPosition};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// The note under the pointer at a point, if any shows.
fn note_at(rig: &common::Rig, tip: &Tip, x: f32, y: f32) -> Option<String> {
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
    rig.pump();
    tip.get_shown().then(|| tip.get_text().to_string())
}

/// The first height, from the top, at which `x` shows `note`.
fn row_of(rig: &common::Rig, tip: &Tip, x: f32, note: &str) -> Option<f32> {
    // A key removed under the pointer never says it was left: its note is cleared by hand.
    tip.set_shown(false);
    (0..400).map(|i| i as f32 * 2.0).find(|&y| note_at(rig, tip, x, y).as_deref() == Some(note))
}

/// Each note met along a line, in order, with the span it covers.
fn notes_along(rig: &common::Rig, tip: &Tip, from: f32, to: f32, y: f32) -> Vec<(String, f32, f32)> {
    let mut met: Vec<(String, f32, f32)> = Vec::new();
    let mut x = from;
    while x < to {
        if let Some(note) = note_at(rig, tip, x, y) {
            match met.last_mut() {
                Some(last) if last.0 == note => last.2 = x,
                _ => met.push((note, x, x)),
            }
        }
        x += 1.0;
    }
    met
}

/// `librarySidebar`'s row: the target key, the layout key, then the pill, on one line.
fn keys_then_pill(met: &[(String, f32, f32)], target: &str, layout: &str, pill: &str) -> bool {
    let names: Vec<&str> = met.iter().map(|m| m.0.as_str()).collect();
    names == [target, layout, pill]
}

/// A plate filling its block: its three keys from the block's one edge to the other.
fn fills_block(met: &[(String, f32, f32)], start: f32, edge: f32) -> bool {
    met.len() == 3 && met[0].1 - start <= 4.0 && edge - met[2].2 <= 4.0
}

/// The sum of a 565 pixel's three channels, each scaled to 8 bits.
fn light(p: Rgb565Pixel) -> u32 {
    let v = p.0 as u32;
    ((v >> 11) & 0x1f) * 255 / 31 + ((v >> 5) & 0x3f) * 255 / 63 + (v & 0x1f) * 255 / 31
}

/// The words a row of keys shows between `from` and `to`, as spans of inked columns across its legend's
/// band; a key's face is read at `from`, and letters closer than a key's padding join one word.
fn words_along(rig: &common::Rig, from: f32, to: f32, top: f32, height: f32) -> Vec<(f32, f32)> {
    let (w, px) = rig.drawn();
    let rows = (top + height / 4.0) as usize..(top + 3.0 * height / 4.0) as usize;
    let face = light(px[(top + height / 2.0) as usize * w + from as usize]);
    let inked = |x: usize| rows.clone().any(|y| light(px[y * w + x]) > face + 120);
    let mut words: Vec<(f32, f32)> = Vec::new();
    for x in from as usize..to as usize {
        if !inked(x) {
            continue;
        }
        match words.last_mut() {
            Some(last) if x as f32 - last.1 <= 8.0 => last.1 = x as f32,
            _ => words.push((x as f32, x as f32)),
        }
    }
    words
}

/// Three words as far from one another, to within a few points.
fn evenly_apart(words: &[(f32, f32)]) -> bool {
    words.len() == 3 && ((words[1].0 - words[0].1) - (words[2].0 - words[1].1)).abs() <= 3.0
}

pub(crate) fn the_sidebars_lay_their_keys_on_plates_as_swift() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the sidebars are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-sidebars-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.5)] {
        common::plain_tiff(&folder.join(name), 96, 64, tone);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, panel, tip) = (w.global::<Library>(), w.global::<PanelState>(), w.global::<Tip>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    rig.shot("sidebars-column");

    // Beside a photo: the target and layout keys on one plate at the left, the pill after them.
    let target = "Show the Selected Photo";
    let layout = lib.get_tree_layout_help().to_string();
    let pill = w.global::<Navigator>().get_opened_help().to_string();
    assert!(!pill.is_empty(), "the opened folder names its path");
    row_of(&rig, &tip, 22.0, target).expect("the target key at the column's left");
    // A key's tooltip waits the system's delay; a help mark's note shows at once, as Swift's does.
    assert!(!tip.get_at_once(), "twin: a tooltip is not shown at once");
    let tips = w.global::<oni_app::Tips>();
    tips.invoke_note(true, "a note".into(), 10.0, 10.0, 0.0);
    assert!(tip.get_shown() && tip.get_at_once(), "a help mark's note shows at once");
    tips.invoke_note(false, "a note".into(), 10.0, 10.0, 0.0);
    assert!(!tip.get_shown());
    let y = row_of(&rig, &tip, 22.0, target).expect("the target key again");
    let met = notes_along(&rig, &tip, 0.0, 260.0, y);
    assert!(keys_then_pill(&met, target, &layout, &pill), "{met:?}");
    assert!(met.iter().all(|m| m.0 != "Read the folders again"), "Swift shows no reload key here");
    assert!(met[1].1 - met[0].2 <= 3.0, "the two keys sit flush on their plate: {met:?}");
    // Twin: the pill alone on its row, the keys right-aligned under it, is refused.
    let apart = [(layout.clone(), 200.0, 224.0), (target.to_string(), 228.0, 252.0)];
    assert!(!keys_then_pill(&apart, target, &layout, &pill));

    // The Crop tab's first plate spans its block, its three keys sharing the room alike.
    panel.set_tab(1);
    panel.invoke_tab_picked(1);
    w.global::<Framing>().invoke_act(oni_app::Act::CopyFraming);
    rig.settle();
    rig.shot("sidebars-crop");
    let (left, right) = (1280.0 - 340.0, 1280.0);
    let copy = "Copies this photo's crop.";
    let y = row_of(&rig, &tip, left + 20.0, copy).expect("COPY at the block's left");
    let met = notes_along(&rig, &tip, left, right, y);
    // The block's edges: the panel's, less the section's inset (`Space.md`).
    let (start, edge) = (left + 16.0, right - 16.0);
    assert!(fills_block(&met, start, edge), "the plate spans the block: {met:?}");
    // Twin: three keys hugging their words at the block's left is no filling plate.
    let hugging = [(copy.to_string(), 956.0, 990.0), ("p".to_string(), 992.0, 1030.0), ("r".to_string(), 1032.0, 1100.0)];
    assert!(!fills_block(&hugging, start, edge));
    tip.set_shown(false);
    let words = words_along(&rig, start + 2.0, edge - 2.0, y, 32.0);
    assert!(evenly_apart(&words), "COPY, PASTE and PASTE TO ROLL evenly apart: {words:?}");
    // Twin: COPY and PASTE hugging their words, PASTE TO ROLL centred in the rest of the plate.
    assert!(!evenly_apart(&[(968.0, 1000.0), (1024.0, 1059.0), (1124.0, 1210.0)]));

    // `ColorPane`: a colour photo offers Auto White Balance; a black and white one has no Color block.
    let balance = "The frame's light toward grey with Temperature and Tint, scaled by Balance strength";
    panel.set_tab(2);
    panel.invoke_tab_picked(2);
    rig.settle();
    rig.shot("sidebars-color");
    assert!(row_of(&rig, &tip, left + 170.0, balance).is_some(), "twin: a colour photo shows the Color block");
    panel.invoke_film_type_picked(2);
    rig.settle();
    rig.shot("sidebars-color-bw");
    assert!(panel.get_monochrome());
    assert_eq!(row_of(&rig, &tip, left + 170.0, balance), None, "a black and white photo has no Color block");
    panel.invoke_film_type_picked(0);
    panel.set_tab(0);
    panel.invoke_tab_picked(0);
    panel.invoke_flat_field_toggled(true);
    lib.invoke_surface_picked(1);
    rig.settle();
    rig.shot("sidebars-roll-projects");
    let _ = std::fs::remove_dir_all(&root);
}
