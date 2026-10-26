//! Entering the export surface on two frames of roll #8: the window shows before any scan is read,
//! and the lines land after.

use oni_app::app::export_card;
use oni_app::app::{Options, Shell};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

const LONG: Duration = Duration::from_secs(600);

/// Two frames of roll #8 with no sidecar, so every line opens on the scan's own facts.
pub(crate) fn entering_the_export_reads_no_scan_on_the_interface_thread() {
    let roll = oni_fixtures::folder(oni_fixtures::RAW_DIR);
    let names = ["DSC_6432.NEF", "DSC_6433.NEF"];
    if !names.iter().all(|n| roll.join(n).exists()) {
        eprintln!("roll #8 is absent: entering the export is not exercised");
        return;
    }
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: entering the export is not exercised"),
    };
    let shots = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots");
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let root = std::env::temp_dir().join(format!("oni-app-export-entering-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Two");
    std::fs::create_dir_all(&folder).unwrap();
    for name in names {
        std::fs::hard_link(roll.join(name), folder.join(name)).unwrap();
    }
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { canvas: false, support: root.join("support"), caches: root.join("caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let targets: Vec<oni_app::export::Target> =
        names.iter().map(|n| oni_app::export::Target { path: folder.join(n), settings: Default::default() }).collect();

    let started = std::time::Instant::now();
    ctl.enter_export(targets.clone());
    let entered = started.elapsed();
    println!("entering the export on {} frames: {:.1} ms", names.len(), entered.as_secs_f64() * 1e3);
    assert!(export_card::card(&ctl).rows.iter().all(Option::is_none), "no scan is read before the surface shows");
    rig.until("the lines", LONG, || export_card::card(&ctl).rows.iter().all(Option::is_some));
    let landed: Vec<String> = export_card::card(&ctl).rows.iter().map(|r| r.as_ref().unwrap().metadata.camera.clone()).collect();
    assert!(landed.iter().all(|c| c == "NIKON CORPORATION NIKON Z f"), "{landed:?}");
    assert!(export_card::lines(&shell.window).iter().all(|l| l.camera == "NIKON CORPORATION NIKON Z f"), "the lines show what landed");
    // Twin: read on this thread, as entering did, the same lines are there before any landing.
    let started = std::time::Instant::now();
    let read: Vec<String> = targets.iter().map(|t| oni_app::export::opening_row(&t.path, None, "", "").metadata.camera).collect();
    println!("the same lines read on this thread: {:.1} ms", started.elapsed().as_secs_f64() * 1e3);
    assert_eq!(read, landed);
    ctl.leave_export();
    rig.settle();
    let _ = std::fs::remove_dir_all(&root);
}
