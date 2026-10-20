//! The transition from the earlier version in the window: a folder holding a sidecar the Swift app
//! wrote opens behind the blocking sheet, its camera is measured through a stand-in for Apple's decode,
//! the frame is corrected once, the editor shows the corrected settings, and a later open asks nothing.

use oni_app::app::{Options, Shell};
use oni_app::{Transition, TransitionLine};
use oni_core::{Archive, EditorStore};
use oni_settings::{ConversionMode, PipelineSettings, sidecar};
use oni_transition::records::{self, State};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model, PhysicalSize};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::common;

const SHIFT: [f64; 3] = [0.063, -0.026, -0.126];
const DAY: f64 = 86_400.0;

/// A frame's sidecar last written `days` ago, as the earlier app or this one left it.
fn written(raw: &Path, days: f64) {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.gains.stops = [-1.0, 0.0, 1.0];
    sidecar::write(&s, raw, sidecar::WriteOptions::default()).unwrap();
    let file = std::fs::OpenOptions::new().write(true).open(sidecar::url_for(raw)).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs_f64(days * DAY)).unwrap();
}

pub(crate) fn an_earlier_roll_is_corrected_once_behind_the_sheet() {
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
    // The originals linked, never copied nor written: only the sidecars beside them are the check's.
    for name in ["DSC_6432.NEF", "DSC_6433.NEF"] {
        std::fs::hard_link(reference.join(name), roll.join(name)).unwrap();
    }
    let (earlier, later) = (roll.join("DSC_6432.NEF"), roll.join("DSC_6433.NEF"));
    written(&earlier, 30.0);
    written(&later, 1.0);
    let installed = State { version: records::VERSION, installed_at: records::now() - 10.0 * DAY, cameras: Vec::new(), deferred: Vec::new() };
    installed.save(&support).unwrap();
    let untouched = std::fs::read(sidecar::url_for(&later)).unwrap();
    let roots = vec![roll.to_string_lossy().into_owned()];
    assert_eq!(oni_transition::pending(&support, &roots), 1, "the sidecar written after the install is not the earlier app's");

    let shots = std::env::var_os("ONI_SHOTS").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let apple: Arc<dyn oni_transition::AppleDecode> = Arc::new(oni_transition::Simulated { stops: SHIFT });
    let options = Options { support: support.clone(), caches: root.join("Caches"), picker: Box::new(|| None), transition: Some(apple), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let sheet = shell.window.global::<Transition>();
    shell.add(&earlier);
    rig.until("the sheet", Duration::from_secs(30), || sheet.get_shown());
    assert!(!sheet.get_done(), "CONTINUE waits for the work");
    rig.shot("transition-running");
    assert!(shell.window.get_sheet_shown(), "the sheet holds the window");
    rig.key("\u{1b}");
    assert!(sheet.get_shown(), "Escape does not close it while it works");
    rig.until("the correction", Duration::from_secs(300), || sheet.get_done());
    rig.settle();
    rig.shot("transition-done");

    let lines: Vec<TransitionLine> = sheet.get_cameras().iter().collect();
    assert_eq!(lines.len(), 1, "one camera");
    assert_eq!(lines[0].camera.as_str(), "Nikon Z f · 1 photo");
    assert_eq!(lines[0].detail.as_str(), "R +0.063  G \u{2212}0.026  B \u{2212}0.126 stop");
    assert_eq!(sheet.get_summary().as_str(), "1 photo corrected.");
    let corrected = sidecar::read(&earlier, true, true).unwrap().settings.gains.stops;
    for c in 0..3 {
        assert!((corrected[c] - ([-1.0, 0.0, 1.0][c] + SHIFT[c] as f32)).abs() < 2e-3, "{corrected:?}");
    }
    assert_eq!(std::fs::read(sidecar::url_for(&later)).unwrap(), untouched, "twin: this version's sidecar is left byte for byte");
    assert_eq!(records::manifest(&support).len(), 1, "the one sidecar corrected is the one backed up");
    // The editor shows the corrected sidecar, whether it opened the frame before the run ended or after.
    let ctl = shell.ctl.clone();
    rig.until("the frame in the editor", Duration::from_secs(60), || ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("DSC_6432.NEF")));
    assert_eq!(ctl.state.borrow().store().settings().gains.stops, corrected);
    assert_ne!(corrected, [-1.0, 0.0, 1.0], "twin: the earlier gains are gone from the editor too");

    sheet.invoke_continue_pressed();
    rig.pump();
    assert!(!sheet.get_shown());
    // Opened again, the folder holds nothing earlier: no sheet, nothing written.
    let written_once = std::fs::read(sidecar::url_for(&earlier)).unwrap();
    shell.open_folder(&roll);
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!sheet.get_shown(), "a corrected roll never asks again");
    assert_eq!(oni_transition::pending(&support, &roots), 0);
    assert_eq!(std::fs::read(sidecar::url_for(&earlier)).unwrap(), written_once);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
