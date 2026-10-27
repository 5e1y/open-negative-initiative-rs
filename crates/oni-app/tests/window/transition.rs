//! The transition from the earlier version in the window: a folder of a camera the table lacks opens
//! behind the blocking sheet, the camera is matched through a stand-in for Apple's decode, every later
//! decode of it folds the matrix, no setting is written, and a later open asks nothing.

use oni_app::app::{Options, Shell};
use oni_app::{Transition, TransitionLine};
use oni_core::{Archive, EditorStore};
use oni_transition::table::Table;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model, PhysicalSize};
use std::sync::Arc;
use std::time::Duration;

use crate::common;

/// The study's Z f matrix, LibRaw's Rec. 2020 to Apple's.
const ZF: [[f64; 3]; 3] = [[1.1252, -0.1698, -0.0064], [0.0259, 0.9537, -0.0640], [0.0092, -0.0109, 0.8969]];

pub(crate) fn a_new_camera_is_matched_once_behind_the_sheet() {
    let reference = oni_fixtures::folder(oni_fixtures::RAW_DIR);
    if !reference.join("DSC_6433.NEF").exists() {
        return println!("SKIPPED the transition: roll #8 is absent");
    }
    if !oni_fixtures::heavy("the transition") {
        return;
    }
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return println!("SKIPPED the transition: {e}"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-transition-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (support, roll) = (root.join("Support"), root.join("Roll"));
    std::fs::create_dir_all(&roll).unwrap();
    // The originals linked, never copied nor written; the sidecar is the check's own.
    for name in ["DSC_6432.NEF", "DSC_6433.NEF"] {
        std::fs::hard_link(reference.join(name), roll.join(name)).unwrap();
    }
    let first = roll.join("DSC_6432.NEF");
    let sidecar = roll.join("DSC_6432.NEF.json");
    std::fs::copy(reference.join("DSC_6432.NEF.json"), &sidecar).unwrap();
    let untouched = std::fs::read(&sidecar).unwrap();
    let roots = vec![roll.to_string_lossy().into_owned()];
    assert_eq!(oni_transition::pending(&support, &roots), 1, "one camera the table lacks");

    let shots = std::env::var_os("ONI_SHOTS").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let apple: Arc<dyn oni_transition::AppleDecode> = Arc::new(oni_transition::Simulated { matrix: ZF });
    let options = Options { support: support.clone(), caches: root.join("Caches"), picker: Box::new(|| None), transition: Some(apple), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let sheet = shell.window.global::<Transition>();
    shell.add(&first);
    rig.until("the sheet", Duration::from_secs(30), || sheet.get_shown());
    assert!(!sheet.get_done(), "CONTINUE waits for the work");
    assert!(shell.window.get_sheet_shown(), "the sheet holds the window");
    rig.shot("transition-running");
    rig.key("\u{1b}");
    assert!(sheet.get_shown(), "Escape does not close it while it works");
    rig.until("the match", Duration::from_secs(300), || sheet.get_done());
    rig.settle();
    rig.shot("transition-done");

    let lines: Vec<TransitionLine> = sheet.get_cameras().iter().collect();
    assert_eq!(lines.len(), 1, "one camera");
    assert_eq!(lines[0].camera.as_str(), "Nikon Z f · 2 photos");
    assert!(lines[0].detail.starts_with("matched, within 0.0 %"), "{}", lines[0].detail);
    assert_eq!(sheet.get_summary().as_str(), "1 camera matched.");
    let table = Table::load(&support).unwrap();
    let m = table.get("Nikon", "Z f").expect("the Z f in the table").matrix;
    assert!((0..3).all(|r| (0..3).all(|c| (m[r][c] - ZF[r][c]).abs() < 2e-3)), "{m:?}");
    assert!(oni_decode::rendition::matrix_for("Nikon", "Z f").is_some(), "installed for every later decode");
    // Twin: the match writes no setting; the frame opened keeps only its balance read ahead, Rust's own key.
    let keys = |bytes: &[u8]| -> Vec<String> {
        let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        v.as_object().unwrap().keys().filter(|k| !["autoBalance", "originalPrint", "originalTail"].contains(&k.as_str())).cloned().collect()
    };
    let now = std::fs::read(&sidecar).unwrap();
    assert_eq!(keys(&now), keys(&untouched), "twin: a key of the sidecar moved");
    let read = |bytes: &[u8]| oni_settings::codec::decode(bytes).unwrap().settings;
    assert_eq!(read(&now), read(&untouched), "twin: no setting is written");
    let ctl = shell.ctl.clone();
    rig.until("the frame in the editor", Duration::from_secs(60), || ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("DSC_6432.NEF")));

    sheet.invoke_continue_pressed();
    rig.pump();
    assert!(!sheet.get_shown());
    // Opened again, the folder holds no camera to match: no sheet.
    shell.open_folder(&roll);
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!sheet.get_shown(), "a matched camera never asks again");
    assert_eq!(oni_transition::pending(&support, &roots), 0);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
