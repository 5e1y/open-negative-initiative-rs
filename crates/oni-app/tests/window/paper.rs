//! The Paper section of the Colour tab in the headless window, open right after More Color Settings: the virtual enlarger
//! offered on a negative, off at its rest; on, it prints on Portra Endura and shows Print on paper, on, Paper blacks, off
//! at rest, the paper and the enlarger's filtration, and Lighting Brightness in place of Colour compensation, each
//! rendering again; Print on paper off prints on the neutral paper, the paper kept, its picker and blacks hidden; another
//! paper picked by its name, the Frontier SP-3000 among them; a slide on the same papers and the neutral one, through its
//! internegative; a digital photo on its paper, its blacks pure.

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

pub(crate) fn the_paper_switch_prints_a_negative_and_a_slide_on_every_paper() {
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
    let count = |role: Role, label: &str| common::placed(w, role, label).len();

    // A negative never prepared rests on ONI's print, the enlarger offered in its own open section, Print on paper hidden.
    assert!(panel.get_paper_shown() && !panel.get_enlarger_on(), "offered, off");
    let switches = || (count(Role::Checkbox, "Virtual enlarger"), count(Role::Checkbox, "Print on paper"), count(Role::Checkbox, "Paper blacks"));
    assert_eq!((switches(), count(Role::Checkbox, "Colour compensation")), ((1, 0, 0), 1));
    section_laid_after_more_color_settings(w);
    rig.settle();
    println!("shot {}", rig.shot("colour-negative-onis-print").display());
    let renders = ctl.state.borrow().renders();
    panel.invoke_enlarger_toggled(true);
    rig.pump();
    assert_eq!((settings().print_paper, settings().paperless), (Some(PaperStock::KodakPortraEndura), false));
    assert!(panel.get_enlarger_on() && panel.get_on_paper() && panel.get_paper_title() == "Kodak Portra Endura");
    assert_eq!(switches(), (1, 1, 1));
    assert!(panel.get_paper_filtration().starts_with("Enlarger filtration: C "), "{}", panel.get_paper_filtration());
    rig.until("the frame printed on the paper", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    // On the paper the section offers Paper blacks, off at rest, and Lighting Brightness; Colour compensation, with
    // nothing to cut there, leaves.
    assert_eq!((count(Role::Slider, "Brightness"), count(Role::Checkbox, "Paper blacks"), count(Role::Checkbox, "Colour compensation")), (1, 1, 0));
    assert!(!settings().print.paper_blacks && !panel.get_paper_blacks(), "pure black at rest, the switch off");
    let (enlarger, paper) = (common::the(w, Role::Checkbox, "Virtual enlarger"), common::the(w, Role::Checkbox, "Print on paper"));
    let blacks = common::the(w, Role::Checkbox, "Paper blacks");
    let filtration = common::the(w, Role::Text, &panel.get_paper_filtration());
    assert!(enlarger.y < paper.y && paper.y < blacks.y && blacks.y < filtration.y, "the switches, the blacks, then the paper's own rows");
    section_laid_after_more_color_settings(w);
    rig.settle();
    println!("shot {}", rig.shot("colour-negative-on-paper").display());
    // Print on paper off: the enlarger prints on the neutral paper, the paper kept as chosen, its picker and blacks gone,
    // the filtration and Brightness kept; on again, the paper's print returns.
    let histogram = || ctl.state.borrow().histogram().cloned();
    let shown_paperless = |paperless: bool| ctl.state.borrow().shown_settings().is_some_and(|s| s.print_paper.is_some() && s.paperless == paperless);
    rig.until("the paper's histogram", Duration::from_secs(60), || shown_paperless(false) && histogram().is_some());
    let (renders, on_paper) = (ctl.state.borrow().renders(), histogram());
    panel.invoke_on_paper_toggled(false);
    rig.pump();
    assert!(settings().paperless && settings().print_paper == Some(PaperStock::KodakPortraEndura) && !panel.get_on_paper());
    assert_eq!((switches(), count(Role::Slider, "Brightness"), count(Role::Checkbox, "Colour compensation")), ((1, 1, 0), 1, 0));
    assert!(panel.get_paper_filtration().starts_with("Enlarger filtration: C "), "{}", panel.get_paper_filtration());
    assert!(common::placed(w, Role::Text, &panel.get_paper_filtration()).len() == 1, "the filtration shown");
    rig.until("the frame printed on the neutral paper", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    // The neutral paper's print is another picture than the paper's, its own histogram.
    rig.until("the neutral paper's own histogram", Duration::from_secs(60), || shown_paperless(true) && histogram().is_some_and(|h| Some(&h) != on_paper.as_ref()));
    section_laid_after_more_color_settings(w);
    rig.settle();
    println!("shot {}", rig.shot("colour-negative-neutral-paper").display());
    let renders = ctl.state.borrow().renders();
    panel.invoke_on_paper_toggled(true);
    rig.pump();
    assert!(!settings().paperless && panel.get_on_paper() && switches() == (1, 1, 1));
    rig.until("the frame printed on its paper again", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    let renders = ctl.state.borrow().renders();
    panel.invoke_paper_blacks_toggled(true);
    rig.pump();
    assert!(settings().print.paper_blacks && panel.get_paper_blacks());
    rig.until("the pick printed on the paper's own black", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    let renders = ctl.state.borrow().renders();
    w.global::<Store>().invoke_step(Field::Brightness.index(), 4);
    rig.pump();
    assert!(settings().print.brightness > 0.0, "{}", settings().print.brightness);
    rig.until("the print brightened", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    let names: Vec<String> = (0..panel.get_paper_lines().row_count()).filter_map(|i| panel.get_paper_lines().row_data(i)).map(|l| l.text.to_string()).collect();
    assert_eq!(names, PaperStock::ALL.iter().map(|p| oni_app::stocks::paper_label(*p).to_string()).collect::<Vec<_>>());
    assert!(names.contains(&"Kodak Endura Premier · Datasheet".to_string()), "the data-sheet twin named by its source");
    let renders = ctl.state.borrow().renders();
    let crystal_archive = PaperStock::ALL.iter().position(|p| *p == PaperStock::FujifilmCrystalArchiveTypeII).unwrap();
    panel.invoke_paper_chosen(crystal_archive as i32);
    rig.pump();
    assert_eq!(settings().print_paper, Some(PaperStock::FujifilmCrystalArchiveTypeII));
    assert_eq!(panel.get_paper_title(), "Fujifilm Crystal Archive Type II");
    rig.until("the frame printed on Crystal Archive", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    // The Frontier SP-3000 is one more paper: picked by its name, the frame prints on it again.
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
    panel.invoke_paper_chosen(crystal_archive as i32);
    rig.pump();

    // A slide prints on the same papers through its internegative: switched to a slide, the frame keeps its paper.
    let renders = ctl.state.borrow().renders();
    panel.invoke_film_type_picked(1);
    rig.pump();
    assert!(panel.get_paper_shown() && panel.get_enlarger_on(), "the slide keeps its paper");
    assert_eq!(settings().print_paper, Some(PaperStock::FujifilmCrystalArchiveTypeII));
    assert_eq!(panel.get_paper_lines().row_count(), PaperStock::ALL.len());
    assert!(panel.get_paper_filtration().starts_with("Enlarger filtration: C "), "{}", panel.get_paper_filtration());
    rig.until("the slide printed through its internegative", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    assert_eq!((count(Role::Slider, "Brightness"), count(Role::Checkbox, "Paper blacks")), (1, 1));
    rig.settle();
    println!("shot {}", rig.shot("slide-on-paper").display());
    // Off, then on again, it takes the default paper.
    panel.invoke_enlarger_toggled(false);
    rig.pump();
    assert!(!panel.get_enlarger_on() && settings().print_paper.is_none());
    panel.invoke_enlarger_toggled(true);
    rig.pump();
    assert_eq!(settings().print_paper, Some(PaperStock::KodakPortraEndura));
    assert!(panel.get_enlarger_on() && panel.get_paper_title() == "Kodak Portra Endura");
    // Print on paper off: the slide's copy prints on the neutral paper, then back on its own.
    let renders = ctl.state.borrow().renders();
    panel.invoke_on_paper_toggled(false);
    rig.pump();
    assert!(settings().paperless && !panel.get_on_paper());
    rig.until("the slide's copy on the neutral paper", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    panel.invoke_on_paper_toggled(true);
    rig.pump();
    assert!(!settings().paperless);
    panel.invoke_enlarger_toggled(false);
    rig.pump();
    assert_eq!(settings().print_paper, None);
    assert!(!panel.get_enlarger_on());
    // On ONI's print Brightness, Print on paper and Paper blacks leave, Colour compensation returns.
    assert_eq!((count(Role::Slider, "Brightness"), switches(), count(Role::Checkbox, "Colour compensation")), (0, (1, 0, 0), 1));
    let _ = std::fs::remove_dir_all(&root);
}

/// The Paper section open, its fold key between More Color Settings' and Lighting's.
fn section_laid_after_more_color_settings(w: &oni_app::MainWindow) {
    let key = |name: &str| common::placed(w, Role::Button, name);
    let (paper, folded) = (key("Hide Paper"), key("Show Paper"));
    assert!(paper.len() == 1 && folded.is_empty(), "the section open: {paper:?} {folded:?}");
    let (more, lighting) = (key("Show More Color Settings"), key("Hide Lighting"));
    assert!(more.len() == 1 && lighting.len() == 1, "{more:?} {lighting:?}");
    assert!(more[0].y < paper[0].y && paper[0].y < lighting[0].y, "{more:?} {paper:?} {lighting:?}");
}

pub(crate) fn a_digital_photo_prints_on_its_paper_on_pure_black() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the digital photo's paper is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-paper-digital-{}", std::process::id()));
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
    let renders = ctl.state.borrow().renders();
    panel.invoke_film_type_picked(3);
    rig.pump();
    assert!(settings().mode.is_digital(), "{:?}", settings().mode);
    rig.until("the photo printed on its film and paper", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    // A digital photo prints on its paper at rest, its blacks pure, the switch shown off.
    assert!(settings().paper().is_some() && panel.get_enlarger_on() && panel.get_on_paper(), "on its paper");
    assert!(!settings().print.paper_blacks && !panel.get_paper_blacks());
    let count = |role: Role, label: &str| common::placed(w, role, label).len();
    assert_eq!((count(Role::Checkbox, "Paper blacks"), count(Role::Checkbox, "Colour compensation")), (1, 0));
    // Twin: the photo's Film block leads the tab, the Paper section still right before Lighting.
    let key = |name: &str| common::placed(w, Role::Button, name);
    let (paper, lighting) = (key("Hide Paper"), key("Hide Lighting"));
    assert!(paper.len() == 1 && lighting.len() == 1 && paper[0].y < lighting[0].y, "{paper:?} {lighting:?}");
    rig.settle();
    println!("shot {}", rig.shot("colour-digital-on-paper").display());
    drop(ctl);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
