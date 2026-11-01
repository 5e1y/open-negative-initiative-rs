//! Nikon's High Efficiency RAW on a click: a converter missing is offered for download, one there
//! converts the folder's HE NEFs beside them, the NEFs go to the bin or stay unlisted, and the editor
//! opens the DNG of the frame it failed on.

use oni_app::app::{Options, Shell};
use oni_app::{Canvas, Converting, DngStage, Library, Notice};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

/// A stand-in for Adobe's program that writes an empty `.dng` for each file after `-d <folder>`.
#[cfg(unix)]
fn stand_in(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let program = dir.join("converter.sh");
    let script = "#!/bin/sh\nwhile [ \"$1\" != \"-d\" ]; do shift; done\nshift; out=\"$1\"; shift\nfor f in \"$@\"; do n=$(basename \"$f\"); : > \"$out/${n%.*}.dng\"; done\n";
    std::fs::write(&program, script).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    program
}

/// The stand-in is a shell script: Windows runs the flow's converter through Adobe's own program alone.
#[cfg(not(unix))]
fn stand_in(_: &Path) -> PathBuf {
    PathBuf::new()
}

pub(crate) fn a_high_efficiency_nef_converts_and_the_editor_opens_its_dng() {
    if cfg!(not(unix)) {
        return eprintln!("the conversion's stand-in is a shell script: not exercised here");
    }
    let Some(samples) = oni_fixtures::present(oni_fixtures::NIKON_HE_DIR, "a_high_efficiency_nef_converts_and_the_editor_opens_its_dng") else { return };
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the conversion is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-dng-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, bin) = (root.join("Scans"), root.join("Bin"));
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    let nef = folder.join("DSC_0001.NEF");
    std::fs::copy(samples.join("Nikon_Z8_high_efficiency_low.NEF"), &nef).unwrap();
    // No converter yet: the sheet offers Adobe's download.
    unsafe { std::env::set_var("ONI_DNG_CONVERTER", root.join("absent")) };
    let rig = common::rig(std::env::temp_dir());
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let binned = bin.clone();
    let options = Options {
        support: root.join("Support"),
        caches: root.join("Caches"),
        picker: Box::new(|| None),
        discard: Box::new(move |p: &Path| std::fs::rename(p, binned.join(p.file_name().unwrap()))),
        ..Options::default()
    };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (canvas, sheet) = (w.global::<Canvas>(), w.global::<Converting>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(60), || ctl.state.borrow().read_done());
    w.global::<Notice>().invoke_close();
    w.global::<Library>().invoke_tile_opened(0);
    rig.until("the HE frame's failure", Duration::from_secs(60), || canvas.get_failure() != "");
    assert!(canvas.get_convertible(), "the editor offers the conversion of an HE frame");
    sheet.invoke_offered();
    rig.pump();
    assert!(sheet.get_shown() && sheet.get_stage() == DngStage::Offer, "no converter: its download is offered");
    sheet.invoke_cancelled();
    rig.pump();
    assert!(!sheet.get_shown());

    // A converter there converts at once, then asks which files stay.
    unsafe { std::env::set_var("ONI_DNG_CONVERTER", stand_in(&root)) };
    sheet.invoke_offered();
    rig.until("the choice", Duration::from_secs(60), || sheet.get_stage() == DngStage::Choice);
    let dng = folder.join("DSC_0001.dng");
    assert!(dng.exists());
    sheet.invoke_kept(true);
    rig.until("the DNG opened", Duration::from_secs(60), || ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("DSC_0001.dng")));
    assert!(bin.join("DSC_0001.NEF").exists() && !nef.exists(), "kept the DNGs only, the NEF went to the bin");
    let listed: Vec<String> = ctl.state.borrow().order().to_vec();
    assert_eq!(listed.len(), 1, "the DNG alone is listed: {listed:?}");

    // Twin: an HE NEF kept beside its DNG stays on the disk and out of the listing.
    std::fs::rename(bin.join("DSC_0001.NEF"), &nef).unwrap();
    ctl.act(oni_app::Act::Refresh);
    rig.until("the folder read again", Duration::from_secs(60), || ctl.state.borrow().read_done());
    rig.settle();
    let listed: Vec<String> = ctl.state.borrow().order().to_vec();
    assert!(nef.exists() && listed.iter().all(|p| !p.ends_with(".NEF")), "the kept NEF is not shown: {listed:?}");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}

