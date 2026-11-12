//! Nikon's High Efficiency RAW: a folder's reading and a roll's entry offer the conversion, the
//! converter's download first when it is missing; the NEFs go to the bin or stay unlisted, and the
//! roll comes back on the DNGs.

use oni_app::app::{Options, Shell};
use oni_app::{Converting, DngStage, Library, Notice};
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

pub(crate) fn high_efficiency_nefs_are_offered_converted_and_their_dngs_taken_up() {
    if cfg!(not(unix)) {
        return eprintln!("the conversion's stand-in is a shell script: not exercised here");
    }
    let Some(samples) = oni_fixtures::present(oni_fixtures::NIKON_HE_DIR, "high_efficiency_nefs_are_offered_converted_and_their_dngs_taken_up") else { return };
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the conversion is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-dng-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, bin) = (root.join("Scans"), root.join("Bin"));
    std::fs::create_dir_all(&bin).unwrap();
    let (first, third) = (folder.join("DSC_0001.NEF"), folder.join("DSC_0003.NEF"));
    common::graded_tiff(&folder.join("DSC_0002.tif"), 96, 64, 0.7);
    std::fs::copy(samples.join("Nikon_Z8_high_efficiency_low.NEF"), &first).unwrap();
    std::fs::copy(samples.join("Nikon_Z8_raw_high_efficiency_hight.NEF"), &third).unwrap();
    // No converter yet: the folder's reading offers Adobe's download.
    unsafe { std::env::set_var("ONI_DNG_CONVERTER", root.join("absent")) };
    let rig = common::rig(std::env::temp_dir());
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let binned = bin.clone();
    let options = Options {
        support: root.join("Support"),
        caches: root.join("Caches"),
        picker: Box::new(|| None),
        require_roll: true,
        discard: Box::new(move |p: &Path| std::fs::rename(p, binned.join(p.file_name().unwrap()))),
        ..Options::default()
    };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let sheet = w.global::<Converting>();
    let listed = || {
        let mut names: Vec<String> = ctl.state.borrow().order().iter().map(|p| Path::new(p).file_name().unwrap().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    };
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(60), || ctl.state.borrow().read_done());
    // RAWs raise the recalibration notice first: the offer waits behind it.
    rig.settle();
    assert!(w.global::<Notice>().get_shown() && !sheet.get_shown(), "the offer waits for the notice to close");
    w.global::<Notice>().invoke_close();
    rig.until("the folder's offer", Duration::from_secs(30), || sheet.get_shown());
    assert!(sheet.get_stage() == DngStage::Offer && !sheet.get_ready(), "the folder's HE NEFs are offered, the converter's download first");
    sheet.invoke_cancelled();
    rig.pump();
    // Twin: the same folder read again does not ask twice.
    ctl.act(oni_app::Act::Refresh);
    rig.until("the folder read again", Duration::from_secs(60), || ctl.state.borrow().read_done());
    rig.settle();
    assert!(!sheet.get_shown(), "the folder offers once a session");

    // A frame opened from an unprepared folder enters its roll, which offers the HE NEFs among its
    // frames; with the converter there the offer converts, then the roll comes back on the DNGs.
    unsafe { std::env::set_var("ONI_DNG_CONVERTER", stand_in(&root)) };
    w.global::<Library>().invoke_tile_opened(0);
    rig.until("the roll's offer", Duration::from_secs(60), || ctl.state.borrow().preparing() && sheet.get_shown());
    assert!(sheet.get_stage() == DngStage::Offer && sheet.get_ready(), "with the converter there, the offer converts");
    sheet.invoke_install();
    rig.until("the choice", Duration::from_secs(60), || sheet.get_stage() == DngStage::Choice);
    assert!(folder.join("DSC_0001.dng").exists() && folder.join("DSC_0003.dng").exists(), "every HE NEF of the roll is converted");
    sheet.invoke_kept(false);
    rig.until("the roll back on its DNGs", Duration::from_secs(60), || {
        ctl.state.borrow().preparing() && !sheet.get_shown() && listed() == ["DSC_0001.dng", "DSC_0002.tif", "DSC_0003.dng"]
    });
    assert!(first.exists() && third.exists(), "the NEF files kept stay on the disk, out of the listing");

    // A failed frame's key converts at once, without asking: here the base step's, the NEF binned.
    std::fs::remove_file(folder.join("DSC_0003.dng")).unwrap();
    ctl.act(oni_app::Act::Refresh);
    rig.until("the NEF listed again", Duration::from_secs(60), || listed().contains(&"DSC_0003.NEF".to_string()));
    sheet.invoke_offered();
    rig.until("the key's choice", Duration::from_secs(60), || sheet.get_shown() && sheet.get_stage() == DngStage::Choice);
    sheet.invoke_kept(true);
    rig.until("the NEF binned", Duration::from_secs(60), || bin.join("DSC_0003.NEF").exists());
    assert!(!third.exists());
    rig.until("the listing without it", Duration::from_secs(60), || listed() == ["DSC_0001.dng", "DSC_0002.tif", "DSC_0003.dng"]);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
