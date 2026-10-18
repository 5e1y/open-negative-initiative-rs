//! The editor's last gestures on the headless window: the Details loupe, the blocks' folds and
//! help, New Roll naming its folder, Copy Settings on a system pasteboard, scroll over a slider.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::pasteboard;
use oni_app::preferences::Preferences;
use oni_app::{DetailLoupe, Editors, Library, PanelState};
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

    // `DSHelpPlacement.place`: a note held Space.sm in from the window's four edges, cut to fit.
    let tips = w.global::<oni_app::Tips>();
    let placed = |x: f32, y: f32, width: f32, height: f32| {
        let b = tips.invoke_place(x, y, width, height, 1280.0, 800.0);
        (b.x, b.y, b.width, b.height)
    };
    assert_eq!(placed(100.0, 120.0, 240.0, 60.0), (100.0, 120.0, 240.0, 60.0), "room enough: where it hangs");
    assert_eq!(placed(1200.0, 780.0, 240.0, 60.0), (1032.0, 732.0, 240.0, 60.0), "past the right and bottom edges");
    assert_eq!(placed(-20.0, 2.0, 240.0, 60.0), (8.0, 8.0, 240.0, 60.0), "past the left and top edges");
    assert_eq!(placed(100.0, 100.0, 2000.0, 900.0), (8.0, 8.0, 1264.0, 784.0), "larger than the room, cut to it");
    // Twin: the note hung at its anchor alone would leave the window.
    assert_ne!(placed(1200.0, 780.0, 240.0, 60.0), (1200.0, 780.0, 240.0, 60.0));

    // A block's fold remembered under `sectionFold.<title>`, the editors' folds read from it at launch.
    assert!(!panel.invoke_fold_of("Texture".into(), false));
    panel.invoke_fold_changed("Texture".into(), true);
    assert!(panel.invoke_fold_of("Texture".into(), false), "the hand's fold wins over the default");
    assert_eq!(Preferences::load(&support).string("sectionFold.Texture"), Some("folded"));
    assert!(panel.invoke_fold_of("Flat-Field".into(), true) && !panel.invoke_fold_of("Sharpening".into(), false), "twin: another block keeps its default");
    assert!(w.global::<Editors>().get_curves_folded(), "Curves opens folded");
    panel.invoke_fold_changed("Curves".into(), false);

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

    // Scroll over a slider: a touch surface's points at a quarter, a wheel's lines at ten points each,
    // and one undo step for the events that come closer than the idle window.
    let store = w.global::<oni_app::Store>();
    let exposure = || oni_app::app::fraction(&store, Field::Exposure);
    let span = 200.0;
    // The step before lands first, past its settle delay and its knob's spring, so the scroll's step is its own.
    rig.settle();
    let rest = exposure();
    store.invoke_scroll(Field::Exposure.index(), 8.0, 0.0, span, false);
    store.invoke_scroll(Field::Exposure.index(), 8.0, 0.0, span, false);
    assert!((exposure() - rest - 2.0 * 8.0 * 0.25 / span as f32).abs() < 1e-5, "{} from {rest}", exposure());
    oni_app::viewport::note_wheel(oni_app::viewport::Wheel { dx: 1.0, dy: 0.0, precise: false });
    let before = exposure();
    store.invoke_scroll(Field::Exposure.index(), 60.0, 0.0, span, false);
    assert!((exposure() - before - 10.0 / span as f32).abs() < 1e-5, "a wheel line is ten points, not Slint's sixty");
    std::thread::sleep(Duration::from_millis(250));
    rig.pump();
    ctl.act("undo");
    rig.settle();
    assert!((exposure() - rest).abs() < 1e-5, "the three events were one step");
    // The hand stays raised until 0.16 s pass without an event; each event restarts that wait.
    let held = || ctl.state.borrow().scrolling();
    store.invoke_scroll(Field::Exposure.index(), 8.0, 0.0, span, false);
    std::thread::sleep(Duration::from_millis(110));
    rig.pump();
    assert!(held(), "inside the idle window");
    store.invoke_scroll(Field::Exposure.index(), 8.0, 0.0, span, false);
    std::thread::sleep(Duration::from_millis(110));
    rig.pump();
    assert!(held(), "twin: the second event restarted the wait, 0.22 s after the first");
    std::thread::sleep(Duration::from_millis(150));
    rig.pump();
    assert!(!held(), "past the idle window");
    ctl.act("undo");
    rig.settle();

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
        assert!(!shell.window.global::<Editors>().get_curves_folded(), "Curves opens as the hand left it");
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

/// The string literals of `text` up to `end`, joined as Swift's `+` joins them.
fn literals(text: &str) -> String {
    text.split('"').skip(1).step_by(2).collect()
}

/// A Swift section's help: the literals between `help:` and the section's content.
fn swift_help(source: &str, title: &str) -> String {
    let at = source.find(&format!("DSSection(\"{title}\"")).unwrap_or_else(|| panic!("{title} in Swift"));
    let from = at + source[at..].find("help:").unwrap();
    let to = from + source[from..].find(") {").unwrap();
    literals(source[from..to].split("separated:").next().unwrap())
}

/// A Slint block's help: the last literal on the `help:` line under its title.
fn slint_help(source: &str, title: &str) -> String {
    let at = source.find(&format!("title: \"{title}\";")).unwrap_or_else(|| panic!("{title} in Slint"));
    let from = at + source[at..].find("help:").unwrap();
    let line = source[from..].lines().next().unwrap();
    line.rsplit('"').nth(1).unwrap_or_default().to_string()
}

/// `DSSection(help:)` on the blocks: each block's note is Swift's own string.
#[test]
fn every_block_says_swifts_own_help() {
    let Some(swift) = oni_app::swift_sources() else { return eprintln!("no Swift checkout: the notes are not compared") };
    // The panel and its tabs' files.
    let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui");
    let mut panel = std::fs::read_to_string(ui.join("panel.slint")).unwrap();
    for f in std::fs::read_dir(ui.join("panel")).unwrap().flatten() {
        panel.push_str(&std::fs::read_to_string(f.path()).unwrap());
    }
    for (file, title) in [
        ("UI/SettingsPane.swift", "Flat-Field"),
        ("UI/SettingsPane.swift", "Dye Separation"),
        ("UI/DetailsPane.swift", "Sharpening"),
        ("UI/DetailsPane.swift", "Chroma Denoise"),
        ("UI/DetailsPane.swift", "Texture"),
    ] {
        let source = std::fs::read_to_string(swift.join(file)).unwrap();
        let want = swift_help(&source, title);
        assert!(want.len() > 20, "{title}: {want}");
        assert_eq!(slint_help(&panel, title), want, "{title}");
        // Twin: one word changed in the port is caught.
        let changed = panel.replacen(&want, &want.replacen(' ', "  ", 1), 1);
        assert_ne!(slint_help(&changed, title), want, "{title}");
    }
    let details = std::fs::read_to_string(swift.join("UI/DetailsPane.swift")).unwrap();
    let off = literals(details.split("offOnSheet = ").nth(1).unwrap().split('\n').take(2).collect::<Vec<_>>().join("").as_str());
    assert!(panel.contains(&format!("off-on-sheet: \"{off}\";")), "{off}");
}

