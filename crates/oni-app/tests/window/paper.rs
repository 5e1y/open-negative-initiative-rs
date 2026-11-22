//! The paper's switch on the Colour tab of the headless window: offered on a negative, off at its rest, on it prints
//! on Portra Endura and shows the enlarger's filtration, Brightness and True blacks in place of Colour compensation,
//! each rendering again; another paper picked by its name, the Frontier SP-3000 among them, none offered on a slide.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Library, PanelState, Store};
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
    rig.window.set_size(slint::PhysicalSize::new(1280, 1600));
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
    // On the paper Lighting offers Brightness and True blacks; Colour compensation, with nothing to cut there, leaves.
    let count = |role: Role, label: &str| common::placed(w, role, label).len();
    assert_eq!((count(Role::Slider, "Brightness"), count(Role::Checkbox, "True blacks"), count(Role::Checkbox, "Colour compensation")), (1, 1, 0));
    let renders = ctl.state.borrow().renders();
    panel.invoke_true_blacks_toggled(true);
    rig.pump();
    assert!(settings().print.true_blacks && panel.get_true_blacks());
    rig.until("the paper's Dmax laid on black", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    let renders = ctl.state.borrow().renders();
    w.global::<Store>().invoke_step(Field::Brightness.index(), 4);
    rig.pump();
    assert!(settings().print.brightness > 0.0, "{}", settings().print.brightness);
    rig.until("the print brightened", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    let names: Vec<String> = (0..panel.get_paper_lines().row_count()).filter_map(|i| panel.get_paper_lines().row_data(i)).map(|l| l.text.to_string()).collect();
    assert_eq!(names, PaperStock::ALL.map(|p| p.name().to_string()));
    let renders = ctl.state.borrow().renders();
    panel.invoke_paper_chosen(4);
    rig.pump();
    assert_eq!(settings().print_paper, Some(PaperStock::FujifilmCrystalArchiveTypeII));
    assert_eq!(panel.get_paper_title(), "Fujifilm Crystal Archive Type II");
    rig.until("the frame printed on Crystal Archive", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    // The Frontier SP-3000 is one more paper: picked by its name, the frame prints on it again.
    let histogram = || ctl.state.borrow().histogram().cloned();
    let shown = |paper: PaperStock| ctl.state.borrow().shown_settings().is_some_and(|s| s.print_paper == Some(paper));
    rig.until("Crystal Archive's histogram", Duration::from_secs(60), || shown(PaperStock::FujifilmCrystalArchiveTypeII) && histogram().is_some());
    let crystal = histogram();
    let renders = ctl.state.borrow().renders();
    let frontier = PaperStock::ALL.iter().position(|p| *p == PaperStock::FujifilmFrontierSp3000).unwrap();
    panel.invoke_paper_chosen(frontier as i32);
    rig.pump();
    assert_eq!(settings().print_paper, Some(PaperStock::FujifilmFrontierSp3000));
    assert_eq!(panel.get_paper_title(), "Fujifilm Frontier SP-3000");
    rig.until("the frame printed on the Frontier", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.until("the Frontier's own histogram", Duration::from_secs(60), || histogram().is_some_and(|h| Some(&h) != crystal.as_ref()));
    // Twin: the same paper picked again writes nothing, so nothing renders anew.
    let (renders, revision) = (ctl.state.borrow().renders(), ctl.state.borrow().store().revision());
    panel.invoke_paper_chosen(frontier as i32);
    rig.pump();
    assert_eq!((ctl.state.borrow().renders(), ctl.state.borrow().store().revision()), (renders, revision));
    panel.invoke_paper_chosen(4);
    rig.pump();

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
    // On ONI's print Brightness and True blacks leave, Colour compensation returns.
    assert_eq!((count(Role::Slider, "Brightness"), count(Role::Checkbox, "True blacks"), count(Role::Checkbox, "Colour compensation")), (0, 0, 1));
    let _ = std::fs::remove_dir_all(&root);
}
