//! Lighting's colour compensation on the Colour tab of the headless window: a switch on at rest, turning it off stores
//! it and renders the frame again; the Saturation Curve's plot draws no print's own curve.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Editors, Library, PanelState, Store};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

pub(crate) fn the_switch_renders_again_and_lays_no_dashed_guide() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the colour compensation switch is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-compensation-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.6);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 1600));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, panel, editors) = (w.global::<Library>(), w.global::<PanelState>(), w.global::<Editors>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    panel.invoke_film_type_picked(0);
    editors.set_saturation_folded(false);
    rig.pump();
    let settings = || ctl.state.borrow().store().settings().clone();
    // The neutral line and the hand's curve: Contrast acts on the negative and lays no curve of its own.
    let lines = || editors.get_saturation_lines().row_count();

    // At rest the switch is on and shown in Lighting.
    assert!(settings().print.colour_compensation && panel.get_colour_compensation());
    common::the(w, Role::Checkbox, "Colour compensation");
    w.global::<Store>().invoke_step(Field::Contrast.index(), 6);
    rig.pump();
    assert_ne!(settings().print.contrast, 0.0);
    assert_eq!(lines(), 2, "no dashed line: the negative lays Contrast's colour");

    let renders = ctl.state.borrow().renders();
    panel.invoke_colour_compensation_toggled(false);
    rig.pump();
    assert!(!settings().print.colour_compensation && !panel.get_colour_compensation());
    rig.until("the frame rendered without the compensation", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    assert_eq!(lines(), 2);
    // Twin: back on, the frame renders again.
    let renders = ctl.state.borrow().renders();
    panel.invoke_colour_compensation_toggled(true);
    rig.pump();
    assert!(settings().print.colour_compensation);
    rig.until("the frame rendered with the compensation", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    let _ = std::fs::remove_dir_all(&root);
}
