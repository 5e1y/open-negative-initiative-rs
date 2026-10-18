//! The Auto White Balance key in the window: pressed on roll #8's first frame, read off the
//! interface thread, laid at Balance Strength's share, and taken back whole by one undo.

use oni_app::PanelState;
use oni_app::app::{Options, Shell};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

/// Roll #8 as hard links beside copies of its sidecars, so no sidecar is written beside the originals.
fn mirror(roll: &Path) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-app-balance-{}", std::process::id())).join("Roll #8");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(roll).unwrap() {
        let from = entry.unwrap().path();
        let to = dir.join(from.file_name().unwrap());
        if from.extension().is_some_and(|e| e == "json") {
            std::fs::copy(&from, &to).unwrap();
        } else {
            std::fs::hard_link(&from, &to).unwrap();
        }
    }
    dir
}

pub(crate) fn the_key_balances_the_open_frame_off_the_interface_thread_and_one_undo_takes_it_back() {
    let roll = oni_fixtures::folder(oni_fixtures::RAW_DIR);
    if !roll.join("DSC_6432.NEF").exists() {
        return println!("SKIPPED the key: roll #8 is absent");
    }
    if !oni_fixtures::heavy("the Auto White Balance key") {
        return;
    }
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return println!("SKIPPED the key: {e}"),
    };
    let rig = common::rig(std::env::temp_dir());
    let window = rig.window.clone();
    let folder = mirror(&roll);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), Options::default()).unwrap();
    window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    window.set_size(PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    shell.open_folder(&folder);
    let ctl = shell.ctl.clone();
    // A folder opens on the gallery; opening its first tile puts the frame in the editor.
    rig.until("the library read", Duration::from_secs(120), || ctl.state.borrow().read_done());
    shell.window.global::<oni_app::Library>().invoke_tile_opened(0);
    rig.until("the first render", Duration::from_secs(300), || ctl.state.borrow().renders() >= 1);
    let open = ctl.state.borrow().open_path().unwrap().to_string();
    assert!(open.ends_with("DSC_6432.NEF"), "{open}");
    let before = ctl.state.borrow().store().settings().clone();
    // Twin: the frame carries no press before the key, so a landing below is the key's own.
    assert!(before.balance_filters.is_none());

    shell.window.global::<PanelState>().invoke_auto_balance();
    rig.until("the press", Duration::from_secs(300), || ctl.state.borrow().store().settings().balance_filters.is_some());
    let after = ctl.state.borrow().store().settings().clone();
    let filters = after.balance_filters.unwrap();
    println!("DSC_6432 balanced to {filters:?}, Balance Strength {}", after.balance_strength);
    assert!((filters[0] + filters[1] + filters[2]).abs() < 1e-5, "the filters carry no common part");
    let laid = [after.print.red.exposure, after.print.green.exposure, after.print.blue.exposure];
    assert!((0..3).all(|c| (laid[c] - filters[c] * after.balance_strength).abs() < 1e-6), "{laid:?} for {filters:?}");
    assert_eq!(after.roll_reference, before.roll_reference, "a frame on its roll keeps its reference");

    ctl.act("undo");
    rig.pump();
    assert_eq!(*ctl.state.borrow().store().settings(), before, "one undo takes the press back whole");
    let _ = std::fs::remove_dir_all(folder.parent().unwrap());
}
