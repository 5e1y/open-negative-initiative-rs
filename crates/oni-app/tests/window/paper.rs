//! The paper's switch on the Colour tab of the headless window: offered on a negative, off at its rest, on it prints
//! on Portra Endura and shows the enlarger's filtration, another paper picked by its name, none offered on a slide.

use oni_app::app::{Options, Shell};
use oni_app::{Library, PanelState};
use oni_core::{Archive, EditorStore};
use oni_settings::PaperStock;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

pub(crate) fn the_paper_switch_prints_a_negative_on_its_paper_and_hides_on_a_slide() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the paper's switch is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-paper-{}", std::process::id()));
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
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, panel) = (w.global::<Library>(), w.global::<PanelState>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.pump();
    let settings = || ctl.state.borrow().store().settings().clone();
    panel.invoke_film_type_picked(0);
    rig.pump();

    // A negative never prepared rests on ONI's print, the switch offered.
    assert!(panel.get_paper_shown() && !panel.get_paper_on(), "offered, off");
    let renders = ctl.state.borrow().renders();
    panel.invoke_paper_toggled(true);
    rig.pump();
    assert_eq!(settings().print_paper, Some(PaperStock::KodakPortraEndura));
    assert!(panel.get_paper_on() && panel.get_paper_title() == "Kodak Portra Endura");
    assert!(panel.get_paper_filtration().starts_with("Enlarger filtration: C "), "{}", panel.get_paper_filtration());
    rig.until("the frame printed on the paper", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    let names: Vec<String> = (0..panel.get_paper_lines().row_count()).filter_map(|i| panel.get_paper_lines().row_data(i)).map(|l| l.text.to_string()).collect();
    assert_eq!(names, PaperStock::ALL.map(|p| p.name().to_string()));
    panel.invoke_paper_chosen(4);
    rig.pump();
    assert_eq!(settings().print_paper, Some(PaperStock::FujifilmCrystalArchiveTypeII));
    assert_eq!(panel.get_paper_title(), "Fujifilm Crystal Archive Type II");

    // A slide is never printed on a negative paper: the switch leaves, the choice kept for a negative.
    panel.invoke_film_type_picked(1);
    rig.pump();
    assert!(!panel.get_paper_shown(), "no paper on a slide");
    assert_eq!(settings().print_paper, Some(PaperStock::FujifilmCrystalArchiveTypeII));
    // Twin: back on a negative, the switch off takes the paper away.
    panel.invoke_film_type_picked(0);
    rig.pump();
    assert!(panel.get_paper_shown() && panel.get_paper_on());
    panel.invoke_paper_toggled(false);
    rig.pump();
    assert_eq!(settings().print_paper, None);
    assert!(!panel.get_paper_on());
    let _ = std::fs::remove_dir_all(&root);
}
