//! The sidebars' keys read off the pointer's notes: beside a photo the column's header is a select in
//! words, a filling group spans its block with its words evenly apart, a key alone on its row spans
//! it too, and a black and white photo has no Color block.

use oni_app::app::{Options, Shell};
use oni_app::{Framing, Library, PanelState, Tip};
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
    (0..500).map(|i| i as f32 * 2.0).find(|&y| note_at(rig, tip, x, y).as_deref() == Some(note))
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

/// A row read without a note: every control on it says what it does in words.
fn worded(met: &[(String, f32, f32)]) -> bool {
    met.is_empty()
}

/// A group filling its block: its keys from the block's one edge to the other.
fn fills_block(met: &[(String, f32, f32)], start: f32, edge: f32) -> bool {
    met.len() > 1 && met[0].1 - start <= 4.0 && met.last().is_some_and(|last| edge - last.2 <= 4.0)
}

/// One key across the row, from the block's one edge to the other and not past it.
fn spans_alone(met: &[(String, f32, f32)], start: f32, edge: f32) -> bool {
    matches!(met, [(_, from, to)] if from - start <= 4.0 && *to <= edge && edge - to <= 4.0)
}

/// One span of ink, the glyph and its word, centred on a key across the block, to within a few points.
fn centred(words: &[(f32, f32)], start: f32, edge: f32) -> bool {
    matches!(words, [(from, to)] if ((from + to) / 2.0 - (start + edge) / 2.0).abs() <= 3.0)
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

/// Swift's `DS.Space.sm` between the keys of one group.
const GROUP_GAP: f32 = 8.0;

/// Each word centred on its key, the group's keys sharing its width alike a group's gap apart, to
/// within a few points.
fn evenly_apart(words: &[(f32, f32)], start: f32, edge: f32) -> bool {
    let n = words.len().max(1) as f32;
    let key = (edge - start - GROUP_GAP * (n - 1.0)) / n;
    words.len() > 1 && words.iter().enumerate().all(|(i, w)| ((w.0 + w.1) / 2.0 - start - (key + GROUP_GAP) * i as f32 - key / 2.0).abs() <= 3.0)
}

pub(crate) fn the_sidebars_lay_their_keys_in_groups_as_swift() {
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

    // Beside a photo the column's header is one select naming its layout in words: no key on its row
    // needs a note to be read.
    let select = common::the(w, i_slint_backend_testing::AccessibleRole::Button, "Two per row");
    let met = notes_along(&rig, &tip, 0.0, 260.0, select.y + select.height / 2.0);
    assert!(worded(&met), "{met:?}");
    // Twin: a row of glyph keys, each read off its note, is refused.
    assert!(!worded(&[("Show the Selected Photo".to_string(), 16.0, 40.0), ("Names only".to_string(), 40.0, 64.0)]));

    // The crop's first group spans its block, its two keys sharing the room alike.
    panel.set_tab(1);
    panel.invoke_tab_picked(1);
    w.global::<Framing>().invoke_act(oni_app::Act::CopyFraming);
    rig.settle();
    rig.shot("sidebars-crop");
    let (left, right) = (1280.0 - 340.0, 1280.0);
    let copy = "Copies this photo's crop: its frame, ratio, angle, turns, mirrors and measuring window.";
    let y = row_of(&rig, &tip, left + 20.0, copy).expect("Copy Crop at the block's left");
    let met = notes_along(&rig, &tip, left, right, y);
    // The block's edges: the panel's, less the section's inset (`Space.md`).
    let (start, edge) = (left + 16.0, right - 16.0);
    assert!(fills_block(&met, start, edge), "the group spans the block: {met:?}");
    // Twin: three keys hugging their words at the block's left is no filling group.
    let hugging = [(copy.to_string(), 956.0, 990.0), ("p".to_string(), 992.0, 1030.0), ("r".to_string(), 1032.0, 1100.0)];
    assert!(!fills_block(&hugging, start, edge));
    tip.set_shown(false);
    let words = words_along(&rig, start + 2.0, edge - 2.0, y, 32.0);
    assert!(evenly_apart(&words, start, edge), "Copy Crop and Paste Crop evenly apart: {words:?}");
    // Twin: keys hugging their words, the last centred in the rest of the group.
    assert!(!evenly_apart(&[(968.0, 1000.0), (1024.0, 1059.0), (1124.0, 1210.0)], start, edge));

    // Rotate & Mirror's keys one a row, as words naming an axis need: each alone spans the block,
    // inside it, its glyph and word centred; the window taller, so the three stand in sight.
    rig.window.set_size(slint::PhysicalSize::new(1280, 1000));
    rig.settle();
    for help in ["Rotate 90° Clockwise", "Mirror left for right", "Mirror top for bottom"] {
        let y = row_of(&rig, &tip, left + 20.0, help).unwrap_or_else(|| panic!("{help} at the block's left"));
        let met = notes_along(&rig, &tip, left, right, y);
        assert!(spans_alone(&met, start, edge), "{help}: its key alone spans the block: {met:?}");
        tip.set_shown(false);
        let words = words_along(&rig, start + 2.0, edge - 2.0, y, 32.0);
        assert!(centred(&words, start, edge), "{help}: its glyph and word centred on the key: {words:?}");
    }
    // Twins: three keys sharing one row, the last pushed past the edge; a word left on its key.
    let row = [("r".to_string(), start, 1040.0), ("m".to_string(), 1041.0, 1150.0), ("v".to_string(), 1151.0, edge + 12.0)];
    assert!(!spans_alone(&row, start, edge) && !spans_alone(&row[2..], start, edge));
    assert!(!centred(&[(start + 12.0, start + 90.0)], start, edge));
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    rig.settle();

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
