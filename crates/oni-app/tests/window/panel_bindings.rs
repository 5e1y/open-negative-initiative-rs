//! The panel's controls by the pointer on the headless window: a switch follows its owner after the
//! hand moved it, Crop and Source Encoding reset, a menu drops under its picker, a typed size box
//! shows what Full resolution writes, and the chip's popover closes before a key opens a window.

use oni_app::app::export_card::{self, Ask};
use oni_app::app::{Options, Shell};
use oni_app::{Exporting, Framing, Library, PanelState};
use oni_core::{Archive, EditorStore};
use oni_settings::SourceEncoding;
use oni_settings::frame::FlatFieldMask;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use crate::common;

fn click(rig: &common::Rig, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

/// A shell on one TIFF naming no profile, its frame open and rendered.
pub(crate) fn opened(name: &str) -> Option<(common::Rig, Shell, PathBuf)> {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => {
            eprintln!("{e}: the panel's controls are not exercised");
            return None;
        }
    };
    let root = std::env::temp_dir().join(format!("oni-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.5);
    std::fs::create_dir_all(support.join("FlatFields")).unwrap();
    std::fs::write(support.join("FlatFields/masks.json"), r#"[{"id":"a1","name":"Rig"}]"#).unwrap();
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
    let lib = shell.window.global::<Library>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    Some((rig, shell, root))
}

pub(crate) fn the_panels_switches_follow_their_owner_and_its_blocks_reset() {
    let Some((rig, shell, root)) = opened("panel-bindings") else { return };
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (panel, framing) = (w.global::<PanelState>(), w.global::<Framing>());
    // Each press measured from the panel's left edge and its tab bar's foot, which the window publishes.
    let at = |x: f32, y: f32| click(&rig, w.get_panel_x() + x, w.get_tabs_foot() + y);
    let settings = || ctl.state.borrow().store().settings().clone();
    let tab = |i: i32| {
        panel.set_tab(i);
        panel.invoke_tab_picked(i);
        rig.settle();
    };

    // Vertical by the pointer, undone from Rust: the next click asks for vertical again, not for
    // the state the hand last left the switch in.
    tab(1);
    framing.invoke_ratio_picked(1);
    rig.settle();
    // `DSToggle`: the switch at the block's left, before its word.
    let vertical = || at(30.0, 292.0);
    vertical();
    rig.settle();
    assert!(settings().geometry.portrait && framing.get_portrait());
    ctl.act(oni_app::Act::Undo);
    rig.settle();
    assert!(!settings().geometry.portrait && !framing.get_portrait(), "the undo takes the switch back");
    vertical();
    rig.settle();
    assert!(settings().geometry.portrait, "the switch followed the undo, so its click turns it on");

    // Crop's RESET: all of the geometry but the measuring window.
    framing.invoke_act(oni_app::Act::WindowArm);
    framing.invoke_act(oni_app::Act::WindowArm);
    rig.settle();
    let window = settings().geometry.measure_window;
    assert!(window.is_some() && framing.get_modified());
    at(261.0, 77.0);
    rig.settle();
    let g = settings().geometry;
    assert_eq!((g.ratio, g.portrait, g.measure_window), (None, false, window), "the window outlives the reset");
    assert!(!framing.get_modified());

    // Source Encoding's RESET writes no encoding, the file read as sRGB again; the window taller, so
    // the keys above leave the block in sight.
    rig.window.set_size(slint::PhysicalSize::new(1280, 900));
    tab(0);
    panel.invoke_encoding_picked(1);
    rig.settle();
    // Flat-Field's chevron: the block unfolds above Source Encoding, its switch then in reach.
    at(311.0, 413.0);
    rig.settle();
    assert_eq!(settings().source_encoding, Some(SourceEncoding::Linear));
    at(261.0, 583.0);
    rig.settle();
    assert_eq!((settings().source_encoding, panel.get_encoding_modified()), (None, false));

    // The flat-field's switch follows an undo too; its mask menu drops under the picker.
    let flat = || at(30.0, 447.0);
    flat();
    rig.settle();
    assert_eq!(settings().flat_field.mask, FlatFieldMask::Custom);
    ctl.act(oni_app::Act::Undo);
    rig.settle();
    assert_eq!(settings().flat_field.mask, FlatFieldMask::None);
    flat();
    rig.settle();
    assert_eq!(settings().flat_field.mask, FlatFieldMask::Custom, "the switch followed the undo");
    at(160.0, 504.0);
    rig.settle();
    rig.shot("bindings-mask-menu");
    at(100.0, 572.0);
    rig.settle();
    assert_eq!(settings().flat_field.custom.as_deref(), Some("a1"), "Rig, the menu's second line, under the picker");
    let _ = std::fs::remove_dir_all(&root);
}

pub(crate) fn the_size_boxes_follow_their_owner_and_the_chip_closes_before_a_window() {
    let Some((rig, shell, root)) = opened("size-boxes") else { return };
    let ctl = shell.ctl.clone();
    let e = shell.window.global::<Exporting>();
    let asked = Rc::new(Cell::new(0));
    let count = asked.clone();
    export_card::answer_with(&ctl, Rc::new(move |_: &Ask| {
        count.set(count.get() + 1);
        Vec::new()
    }));
    let named = |key: Key| slint::SharedString::from(key);

    // 48 typed into W; Full resolution then writes 96, which the box shows, so Return asks nothing.
    ctl.act(oni_app::Act::Export);
    rig.settle();
    // The field's click selects its value, so the digits typed replace it.
    let width_box = || click(&rig, 952.0, 584.0);
    width_box();
    rig.key(&named(Key::Delete));
    rig.key(&named(Key::Delete));
    rig.key("48");
    rig.key(&named(Key::Return));
    rig.settle();
    assert_eq!((e.get_width_text().as_str(), e.get_resized()), ("48", true));
    rig.shot("bindings-export-typed");
    // Full resolution, the key beside the box once it is resized.
    click(&rig, 1112.0, 584.0);
    rig.settle();
    assert_eq!((e.get_width_text().as_str(), e.get_resized()), ("96", false));
    width_box();
    rig.key(&named(Key::Return));
    rig.settle();
    assert!(!e.get_resized(), "the box shows 96, not the 48 typed before Full resolution");

    // 8000 is past the full edge, so Full resolution stays and the owner republishes the same 96:
    // the box shows it again, so a height typed next is the edge that decides.
    width_box();
    rig.key("8000");
    rig.key(&named(Key::Return));
    rig.settle();
    assert_eq!((e.get_width_text().as_str(), e.get_resized()), ("96", false), "8000 is refused");
    let height_box = || click(&rig, 1034.0, 584.0);
    height_box();
    rig.key("32");
    rig.key(&named(Key::Return));
    rig.settle();
    assert_eq!((e.get_height_text().as_str(), e.get_resized()), ("32", true), "the height typed after a refused width resizes");
    // Twin: a width the photo holds still decides, the height following its ratio.
    click(&rig, 1112.0, 584.0);
    rig.settle();
    width_box();
    rig.key("48");
    rig.key(&named(Key::Return));
    rig.settle();
    assert_eq!((e.get_width_text().as_str(), e.get_height_text().as_str()), ("48", "32"));

    // Load a Profile… in the chip's popover closes it before the file window opens.
    // CANCEL, then the space chip over the canvas, then its popover's Custom tab.
    click(&rig, 943.0, 749.0);
    rig.settle();
    click(&rig, 600.0, 116.0);
    rig.settle();
    click(&rig, 739.0, 200.0);
    rig.settle();
    if !e.get_on_profile() {
        eprintln!("no profile installed: the popover's Load a Profile… is not exercised");
        let _ = std::fs::remove_dir_all(&root);
        return;
    }
    let title = e.get_chip_title();
    click(&rig, 504.0, 352.0);
    rig.settle();
    assert_eq!(asked.get(), 1, "the key asked for files");
    click(&rig, 460.0, 200.0);
    rig.settle();
    assert_eq!(e.get_chip_title(), title, "the popover closed, its sRGB tab gone from under the pointer");
    let _ = std::fs::remove_dir_all(&root);
}
