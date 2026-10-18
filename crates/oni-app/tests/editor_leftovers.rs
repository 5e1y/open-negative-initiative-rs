//! The editor's remaining gestures on the headless window: the Details loupe over the frame's full
//! resolution and its ratio badge, New Roll naming its folder in the column, Copy Settings on a
//! system pasteboard under the app's own type.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::pasteboard;
use oni_app::{DetailLoupe, Library, PanelState};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::time::Duration;

mod common;

#[test]
fn the_editors_last_gestures_behave_as_swifts() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the editor's gestures are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-leftovers-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.5)] {
        common::graded_tiff(&folder.join(name), 900, 600, tone);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    // A named system board on the Mac, so the copy crosses a relaunch without touching the clipboard.
    let board_name = format!("oni-leftovers-{}", std::process::id());
    let board = || -> Box<dyn pasteboard::Board> {
        #[cfg(target_os = "macos")]
        return pasteboard::named(&board_name);
        #[cfg(not(target_os = "macos"))]
        Box::new(pasteboard::Memory::default())
    };
    let options = Options { support: support.clone(), caches: caches.clone(), picker: Box::new(|| None), pasteboard: board(), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine.clone(), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, panel, band) = (w.global::<Library>(), w.global::<PanelState>(), w.global::<DetailLoupe>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);

    // The loupe: Details builds the full frame, then shows the band's region at 1.5 drawable pixels a source pixel.
    assert!(!ctl.state.borrow().loupe_shown(), "nothing under Color");
    panel.set_tab(3);
    panel.invoke_tab_picked(3);
    rig.settle();
    rig.until("the loupe", Duration::from_secs(60), || {
        rig.pump();
        ctl.state.borrow().loupe_shown()
    });
    rig.shot("leftovers-loupe");
    let (bw, bh) = (band.get_band_width(), band.get_band_height());
    assert!(bw > 100.0 && bh > 50.0, "the band is laid: {bw} x {bh}");
    let source = band.get_source().size();
    assert_eq!((source.width, source.height), ((bw / 1.5).floor() as u32, (bh / 1.5).floor() as u32), "the band's source pixels at rest");
    assert!((band.get_shown_width() - source.width as f32 * 1.5).abs() < 0.01);
    assert_eq!(band.get_ratio().as_str(), "150%");
    assert!(band.get_ratio_shown());
    // The badge swaps rest for close: fewer source pixels, each drawn larger.
    band.invoke_toggled();
    rig.until("the loupe at 200 %", Duration::from_secs(60), || {
        rig.pump();
        band.get_source().size().width == (bw / 2.0).floor() as u32
    });
    assert_eq!(band.get_ratio().as_str(), "200%");
    assert_ne!(band.get_source().size().width, source.width, "twin: the ratio moved the region");
    band.invoke_toggled();
    assert_eq!(ctl.state.borrow().loupe_magnification(), 1.5);

    // New Roll from Selection: B moved into a dated roll, whose name the column beside the photo asks.
    ctl.act("toggle-mode");
    rig.pump();
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_tile_clicked(1, false, false);
    rig.pump();
    ctl.act("new-roll");
    let scans = PathBuf::from(oni_library::paths::canonical(&folder));
    let dated = oni_library::roll_folder::dated(oni_settings::Date::now());
    assert!(scans.join(&dated).join("B.tif").exists(), "B moved into the roll");
    rig.until("the roll read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let renaming = || {
        let rows = lib.get_rows();
        (0..rows.row_count()).find(|&i| rows.row_data(i).is_some_and(|r| r.renaming))
    };
    assert_eq!(renaming(), None, "the gallery's column leaves the request waiting");
    ctl.act("toggle-mode");
    rig.pump();
    let row = renaming().expect("the roll's row is named in place beside the photo");
    assert_eq!(lib.get_rows().row_data(row).unwrap().title.as_str(), dated);
    rig.settle();
    rig.shot("leftovers-new-roll");
    // A name typed over it renames the folder on disk and names its roll.
    lib.invoke_row_rename_committed(row as i32, "  Holiday \n".into());
    rig.until("the roll renamed", Duration::from_secs(30), || ctl.state.borrow().read_done());
    assert!(scans.join("Holiday/B.tif").exists() && !scans.join(&dated).exists());
    assert_eq!(oni_library::folders::roll(&scans.join("Holiday").to_string_lossy()).map(|f| f.name), Some("Holiday".to_string()));

    // Copy Settings: the codec's text on the board under the app's type, the framing's type untouched.
    panel.set_tab(2);
    panel.invoke_tab_picked(2);
    w.global::<oni_app::Store>().invoke_step(Field::Exposure.index(), 5);
    rig.pump();
    let donor = ctl.state.borrow().store().get(Field::Exposure);
    ctl.act("copy");
    let seen = board();
    let text = String::from_utf8(seen.data(pasteboard::SETTINGS).expect("the settings on the board")).unwrap();
    let decoded = oni_settings::codec::decode(text.as_bytes()).unwrap().settings;
    assert!((Field::Exposure.knob().get(&decoded) - donor).abs() < 1e-6, "the board holds the frame's settings");
    assert_eq!(seen.data(pasteboard::FRAMING), None, "twin: Copy Settings writes no framing");

    // A relaunch: a fresh store on the same board pastes the copy onto B.
    #[cfg(target_os = "macos")]
    {
        drop(shell);
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let options = Options { support, caches, picker: Box::new(|| None), pasteboard: board(), ..Options::default() };
        let shell = Shell::new(Box::new(store), engine, options).unwrap();
        shell.window.show().unwrap();
        let ctl = shell.ctl.clone();
        let lib = shell.window.global::<Library>();
        shell.open_folder(&folder);
        rig.until("the folder read again", Duration::from_secs(30), || ctl.state.borrow().read_done());
        lib.invoke_tile_clicked(0, false, false);
        lib.invoke_tile_clicked(0, false, false);
        rig.until("a frame rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
        // The donor is B: the paste lands on A.
        if ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("B.tif")) {
            std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
            lib.invoke_tile_clicked(1, false, false);
            lib.invoke_tile_clicked(1, false, false);
            rig.until("A rendered again", Duration::from_secs(120), || ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("A.tif")) && ctl.state.borrow().renders() >= 2);
        }
        let before = ctl.state.borrow().store().get(Field::Exposure);
        assert!((before - donor).abs() > 1e-3, "B starts elsewhere");
        ctl.act("paste");
        rig.pump();
        assert!((ctl.state.borrow().store().get(Field::Exposure) - donor).abs() < 1e-6, "the copy crossed the relaunch");
    }
}
