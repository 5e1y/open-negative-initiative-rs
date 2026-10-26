//! The info sidebar's roll guide on the headless window: photos outside any roll guided into one,
//! the whole folder made a roll in place, part of it moved into a new one, and the stock's dyes
//! written on every frame of the roll as one step.

use oni_app::app::{Options, Shell};
use oni_app::{Confirm, Library, RollGuide, Stocks};
use oni_core::{Archive, EditorStore};
use oni_library::paths;
use oni_settings::blocks::SeparationFamily;
use oni_settings::film::FILM_STOCKS;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::PipelineSettings;
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

fn listed(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder).map(|l| l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    names.sort();
    names
}

fn family(path: &Path) -> Option<SeparationFamily> {
    sidecar::read(path, false, true).unwrap().settings.separation_family
}

fn stock(id: &str) -> i32 {
    FILM_STOCKS.iter().position(|k| k.id == id).unwrap() as i32
}

pub(crate) fn photos_outside_a_roll_are_guided_into_one_and_its_stock_writes_their_dyes() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the roll guide is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-roll-guide-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for file in ["Scans/A.tif", "Scans/B.tif", "Scans/C.tif", "Scans/Other/D.tif", "Scans/Other/E.tif"] {
        common::linear_tiff(&root.join(file), 48, 32, 0.5);
    }
    let root = PathBuf::from(paths::canonical(&root));
    let scans = root.join("Scans");
    let other = scans.join("Other");
    let slide = PipelineSettings { separation_family: Some(SeparationFamily::E6), ..PipelineSettings::default() };
    sidecar::write(&slide, &other.join("D.tif"), WriteOptions::default()).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    shell.open_folder(&scans);
    rig.until("the photos read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().order().len() == 5);
    let (lib, stocks, confirm) = (w.global::<Library>(), w.global::<Stocks>(), w.global::<Confirm>());
    let guide = w.global::<RollGuide>();
    let at = |name: &str| ctl.state.borrow().order().iter().position(|p| p.ends_with(&format!("/{name}"))).unwrap() as i32;
    let pick = |names: &[&str]| {
        std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
        lib.invoke_tile_clicked(at(names[0]), false, false);
        for name in &names[1..] {
            lib.invoke_tile_clicked(at(name), true, false);
        }
        rig.pump();
    };
    let frames = ["A.tif", "B.tif", "C.tif"].map(|n| scans.join(n));
    let families = || frames.iter().map(|p| family(p)).collect::<Vec<_>>();

    // Photos of two folders: the guide shows, its key refused with the reason.
    pick(&["A.tif", "D.tif"]);
    assert_eq!(lib.get_info_kind(), 0);
    assert!(guide.get_shown(), "a selection outside any roll shows the guide");
    assert_eq!(guide.get_count().as_str(), "2 selected · 2 folders");
    assert!(!guide.get_enabled() && guide.get_note().contains("2 folders"), "{}", guide.get_note());

    // Part of a folder: New Roll's key; the whole folder: the folder made a roll in place.
    pick(&["A.tif"]);
    assert!(guide.get_enabled() && guide.get_note().is_empty());
    assert_eq!(guide.get_key().as_str(), "Move into a new roll");
    pick(&["A.tif", "B.tif", "C.tif"]);
    assert_eq!(guide.get_key().as_str(), "Make this folder a roll");
    assert_eq!(guide.get_name(), guide.get_proposed(), "prefilled with the date New Roll would give");
    guide.set_name("Portra Trip".into());
    guide.invoke_name_edited("Portra Trip".into());
    guide.invoke_pick_stock();
    rig.pump();
    assert!(stocks.get_shown());
    stocks.invoke_chosen(stock("kodak-portra"));
    rig.pump();
    assert_eq!(guide.get_stock().as_str(), "Kodak Portra");
    assert_eq!(guide.get_name().as_str(), "Portra Trip", "the name typed stays");
    let before = listed(&scans);
    guide.invoke_create();
    rig.pump();
    let film = oni_library::folders::roll(&scans.to_string_lossy()).expect("the folder made a roll");
    assert_eq!((film.name.as_str(), film.stock.as_deref()), ("Portra Trip", Some("kodak-portra")));
    let mut after = before.clone();
    after.push(".oni-roll.json".into());
    after.sort();
    let originals: Vec<String> = listed(&scans).into_iter().filter(|n| !n.ends_with(".tif.json")).collect();
    assert_eq!(originals, after, "no file moves, the sidecars written beside their photos");
    assert!(scans.join("Portra Trip").symlink_metadata().is_err(), "the folder keeps its name");
    assert_eq!(families(), vec![Some(SeparationFamily::KodakC41); 3], "Portra writes Kodak's dyes on every frame");
    assert_eq!(family(&other.join("D.tif")), Some(SeparationFamily::E6), "twin: a frame outside the roll keeps its family");
    assert_eq!((lib.get_info_kind(), guide.get_shown()), (1, false), "twin: inside a roll, the roll card shows");
    assert_eq!(lib.get_info_title().as_str(), "Portra Trip");
    assert_eq!(lib.get_stock_dyes().as_str(), "Dyes: Kodak");
    rig.until("the fresh roll told unmeasured", Duration::from_secs(10), || {
        rig.pump();
        lib.get_roll_unmeasured()
    });
    rig.settle();
    println!("shot {}", rig.shot("roll-guide-made").display());

    // One undo takes the marker and the dyes back, and the guide returns; redo makes it again.
    ctl.act(oni_app::Act::Undo);
    rig.landed(&ctl);
    assert_eq!(oni_library::folders::roll(&scans.to_string_lossy()), None);
    assert_eq!(families(), vec![None; 3]);
    assert!(lib.get_info_kind() == 0 && guide.get_shown());
    ctl.act(oni_app::Act::Redo);
    rig.landed(&ctl);
    assert_eq!(families(), vec![Some(SeparationFamily::KodakC41); 3]);
    assert_eq!(lib.get_info_kind(), 1);

    // Superia picked on the roll card: Fuji's dyes on its three frames in one step, undone as one.
    lib.invoke_select_roll_type();
    stocks.invoke_chosen(stock("fujifilm-superia"));
    rig.pump();
    assert_eq!(families(), vec![Some(SeparationFamily::FujiC41); 3]);
    assert_eq!(family(&other.join("D.tif")), Some(SeparationFamily::E6));
    assert_eq!(lib.get_stock().as_str(), "Fujifilm Superia");
    ctl.act(oni_app::Act::Undo);
    rig.landed(&ctl);
    assert_eq!(families(), vec![Some(SeparationFamily::KodakC41); 3], "one undo gives the families back");
    assert_eq!(lib.get_stock().as_str(), "Kodak Portra", "and the stock");

    // A black-and-white stock leaves every family alone; a measured frame takes the banner away.
    let measured = frames[1].clone();
    let mut b = sidecar::read(&measured, false, true).unwrap().settings;
    b.roll_reference = Some(oni_settings::frame::RollReference { id: oni_settings::Id::new(), stops: [0.0; 3], black: [0.1; 3], white: [0.6; 3], base: None, median: None });
    sidecar::write(&b, &measured, WriteOptions::default()).unwrap();
    lib.invoke_select_roll_type();
    stocks.invoke_chosen(stock("ilford-hp5"));
    rig.pump();
    assert_eq!(families(), vec![Some(SeparationFamily::KodakC41); 3], "black and white writes no dyes");
    assert_eq!(lib.get_stock_dyes().as_str(), "");
    rig.until("the measured roll's banner gone", Duration::from_secs(10), || {
        rig.pump();
        !lib.get_roll_unmeasured()
    });

    // Part of a folder moved into a new roll under the name and stock typed, its dyes following.
    pick(&["D.tif"]);
    assert_eq!(guide.get_key().as_str(), "Move into a new roll");
    guide.set_name("Split".into());
    guide.invoke_name_edited("Split".into());
    guide.invoke_pick_stock();
    stocks.invoke_chosen(stock("cinestill-800t"));
    rig.pump();
    guide.invoke_create();
    rig.roll_confirmed(&ctl, w);
    assert!(!confirm.get_shown());
    let split = other.join("Split");
    rig.until("the frame moved in", Duration::from_secs(30), || split.join("D.tif").exists());
    assert!(other.join("E.tif").exists(), "the frame left unpicked stays");
    let film = oni_library::folders::roll(&split.to_string_lossy()).expect("the new roll's marker");
    assert_eq!((film.name.as_str(), film.stock.as_deref()), ("Split", Some("cinestill-800t")));
    assert_eq!(family(&split.join("D.tif")), Some(SeparationFamily::Ecn2));
    assert_eq!(family(&other.join("E.tif")), None, "twin: the frame left out keeps its family");

    // The frame open in the editor takes the roll's new dyes at once, its Roll tab with it.
    let renders = ctl.state.borrow().renders();
    lib.invoke_tile_opened(at("A.tif"));
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() > renders);
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    pick(&["A.tif"]);
    lib.invoke_select_roll_type();
    stocks.invoke_chosen(stock("fujifilm-superia"));
    rig.pump();
    assert_eq!(ctl.state.borrow().store().settings().separation_family, Some(SeparationFamily::FujiC41));
    assert_eq!(w.global::<oni_app::PanelState>().get_dye_family(), 1, "the Dyes tabs read Fuji");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
