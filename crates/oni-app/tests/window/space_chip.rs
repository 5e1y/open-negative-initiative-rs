//! The canvas's colour-space chip by the pointer: its popover opens named as the export's field and
//! shared with it, and each tab inside it takes the click, Custom and sRGB from Display P3 included.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::{Exporting, Library};
use oni_core::{Archive, EditorStore};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

fn click(rig: &common::Rig, x: f32, y: f32) {
    let position = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

pub(crate) fn the_space_chips_tabs_take_the_pointer() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the space chip is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-space-chip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.5);
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
    let (lib, e) = (w.global::<Library>(), w.global::<Exporting>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    assert_eq!(e.get_chip_title(), "Display P3");
    // The chip's title opens the popover, which stays open while its tabs are picked.
    click(&rig, 600.0, 108.0);
    rig.settle();
    // The export panel's name, and the choice said to be the export's; twin: the former name is gone.
    common::in_popup(w, Role::Text, "Colour space");
    common::in_popup(w, Role::Text, "Shared by the canvas and the export.");
    assert!(common::placed(w, Role::Text, "Working Space").is_empty());
    rig.shot("space-chip");
    // Each tab where the popover lays it, found anew: a press on one leaves the popover open.
    let tab = |rig: &common::Rig, name: &str| {
        let (x, y) = common::in_popup(w, Role::Tab, name).middle();
        click(rig, x, y);
        rig.settle();
    };
    tab(&rig, "sRGB");
    assert_eq!(e.get_chip_title(), "sRGB", "sRGB from Display P3");
    tab(&rig, "P3");
    assert_eq!(e.get_chip_title(), "Display P3");
    // Last: a longer title widens the chip into its room, which the popover under it follows.
    tab(&rig, "Custom");
    assert_eq!(e.get_space_tab(), 4, "Custom from Display P3");
    // Twin: a click beside the popover closes it and picks nothing.
    let (x, y) = common::in_popup(w, Role::Tab, "sRGB").middle();
    click(&rig, 600.0, 108.0);
    rig.settle();
    click(&rig, 600.0, 600.0);
    rig.settle();
    click(&rig, x, y);
    assert_eq!(e.get_space_tab(), 4, "the popover closed, the tabs under it gone");
}
