//! The Paper section of the Colour tab in the headless window, open right after More Color Settings: a negative printed
//! on the default paper, ONI Standard, with no switch to turn the enlarger or the paper off; Print lift, off at rest and
//! rendering anew each way it turns, the paper and the enlarger's filtration, and Lighting Brightness in place of Colour
//! compensation; another paper picked by its name, the Frontier SP-3000 among them; a slide projected, the Paper section
//! gone and the Projector's Projected blacks in its place; a digital photo on its paper, its ends pure.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{Library, PanelState, Store};
use oni_core::{Archive, EditorStore};
use oni_settings::PaperStock;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};
use std::time::Duration;

use crate::common;

pub(crate) fn a_negative_prints_on_every_paper_and_a_slide_is_projected() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the paper is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-paper-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), 96, 64, 0.6);
    let rig = common::rig();
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

    // A negative never prepared prints on the default paper, ONI Standard, in its own open section: no switch turns the
    // enlarger or the paper off; Print lift, off at rest, the paper and the enlarger's filtration, and Lighting Brightness
    // in place of Colour compensation.
    let histogram = || ctl.state.borrow().histogram().cloned();
    assert!(panel.get_paper_shown() && panel.get_paper_title() == "ONI Standard", "{}", panel.get_paper_title());
    assert_eq!(settings().paper(), Some(PaperStock::default()));
    let switches = || (count(Role::Checkbox, "Virtual enlarger"), count(Role::Checkbox, "Print on paper"), count(Role::Checkbox, "Print lift"));
    assert_eq!(switches(), (0, 0, 1));
    assert_eq!((count(Role::Slider, "Brightness"), count(Role::Checkbox, "Colour compensation")), (1, 0));
    assert!(!settings().print.print_lift && !panel.get_print_lift(), "pure ends at rest, the switch off");
    assert!(panel.get_paper_filtration().starts_with("Enlarger filtration: C "), "{}", panel.get_paper_filtration());
    let lift = common::the(w, Role::Checkbox, "Print lift");
    let filtration = common::the(w, Role::Text, &panel.get_paper_filtration());
    assert!(lift.y < filtration.y, "Print lift, then the paper's own rows");
    section_laid_after_more_color_settings(w);
    rig.settle();
    println!("shot {}", rig.shot("colour-negative-on-oni-standard").display());
    rig.until("the paper's histogram", Duration::from_secs(60), || ctl.state.borrow().shown_settings().is_some() && histogram().is_some());
    let renders = ctl.state.borrow().renders();
    let pure = histogram();
    panel.invoke_print_lift_toggled(true);
    rig.pump();
    assert!(settings().print.print_lift && panel.get_print_lift());
    rig.until("the print on the paper's own ends", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    // The paper's own black and white are another picture than pure black and pure white, its own histogram.
    let lifted = |on: bool| ctl.state.borrow().shown_settings().is_some_and(|s| s.print.print_lift == on);
    rig.until("the paper's own ends' histogram", Duration::from_secs(60), || lifted(true) && histogram().is_some_and(|h| Some(&h) != pure.as_ref()));
    println!("shot {}", rig.shot("colour-negative-print-lift").display());
    let renders = ctl.state.borrow().renders();
    panel.invoke_print_lift_toggled(false);
    rig.pump();
    assert!(!settings().print.print_lift && !panel.get_print_lift());
    rig.until("the print on pure ends again", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    let renders = ctl.state.borrow().renders();
    panel.invoke_print_lift_toggled(true);
    rig.pump();
    rig.until("the print on the paper's own ends again", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    let renders = ctl.state.borrow().renders();
    w.global::<Store>().invoke_step(Field::Brightness.index(), 4);
    rig.pump();
    assert!(settings().print.brightness > 0.0, "{}", settings().print.brightness);
    rig.until("the print brightened", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    let (mut names, mut listed) = (papers_listed(&panel), PaperStock::all().iter().map(|p| p.name().to_string()).collect::<Vec<_>>());
    names.sort();
    listed.sort();
    assert_eq!(names, listed, "every paper once");
    let renders = ctl.state.borrow().renders();
    let crystal_archive = PaperStock::all().iter().position(|p| *p == PaperStock::of("fujifilm-crystal-archive-digital-dpii")).unwrap();
    panel.invoke_paper_chosen(crystal_archive as i32);
    rig.pump();
    assert_eq!(settings().print_paper, Some(PaperStock::of("fujifilm-crystal-archive-digital-dpii")));
    assert_eq!(panel.get_paper_title(), "Fujifilm Crystal Archive Digital Type DP II");
    rig.until("the frame printed on Crystal Archive", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    // The Frontier SP-3000 is one more paper: picked by its name, the frame prints on it again.
    let shown = |paper: PaperStock| ctl.state.borrow().shown_settings().is_some_and(|s| s.print_paper == Some(paper));
    rig.until("Crystal Archive's histogram", Duration::from_secs(60), || shown(PaperStock::of("fujifilm-crystal-archive-digital-dpii")) && histogram().is_some());
    let crystal = histogram();
    let renders = ctl.state.borrow().renders();
    let frontier = PaperStock::all().iter().position(|p| *p == PaperStock::of("fujifilm-frontier-sp-3000")).unwrap();
    panel.invoke_paper_chosen(frontier as i32);
    rig.pump();
    assert_eq!(settings().print_paper, Some(PaperStock::of("fujifilm-frontier-sp-3000")));
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

    // A slide is projected, never printed: switched to a slide, the Paper section and Colour compensation leave, the
    // Projector section shows its film and lamp and Projected blacks, off at rest; the paper stays stored for a negative.
    let renders = ctl.state.borrow().renders();
    panel.invoke_film_type_picked(1);
    rig.pump();
    assert!(!panel.get_paper_shown() && panel.get_projected(), "a slide takes no paper");
    assert_eq!(settings().print_paper, Some(PaperStock::of("fujifilm-crystal-archive-digital-dpii")));
    assert!(settings().paper().is_none() && oni_pipeline::conversion::stage47(&settings()).paper.is_none());
    rig.until("the slide projected", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.pump();
    assert_eq!((switches(), count(Role::Slider, "Brightness"), count(Role::Checkbox, "Colour compensation")), ((0, 0, 0), 0, 0));
    assert_eq!(count(Role::Checkbox, "Projected blacks"), 1);
    assert!(!panel.get_projected_blacks() && !settings().print.projected_blacks, "off at rest");
    assert_eq!(panel.get_projector_note(), "Kodak Ektachrome under a 3200 K projector lamp");
    rig.settle();
    println!("shot {}", rig.shot("slide-projected").display());
    // On, the screen's own black and white, rendered anew; off again, back on pure ends.
    let renders = ctl.state.borrow().renders();
    panel.invoke_projected_blacks_toggled(true);
    rig.pump();
    assert!(settings().print.projected_blacks && panel.get_projected_blacks());
    rig.until("the slide on its screen's own black", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    println!("shot {}", rig.shot("slide-projected-blacks").display());
    panel.invoke_projected_blacks_toggled(false);
    rig.pump();
    assert!(!settings().print.projected_blacks);
    // Twin: back on a negative, the Projector leaves and the paper it kept prints again.
    panel.invoke_film_type_picked(0);
    rig.pump();
    assert!(!panel.get_projected() && panel.get_paper_shown());
    assert_eq!(count(Role::Checkbox, "Projected blacks"), 0);
    let _ = std::fs::remove_dir_all(&root);
}

/// The papers the menu lists as it drops, by name, the makers' lines left out.
fn papers_listed(panel: &PanelState) -> Vec<String> {
    panel.invoke_paper_dropping();
    let lines = panel.get_paper_lines();
    (0..lines.row_count()).filter_map(|i| lines.row_data(i)).filter(|l| !l.brand).map(|l| l.name.to_string()).collect()
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
    let rig = common::rig();
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
    // A digital photo prints on its paper at rest, its ends pure, Print lift shown off.
    assert!(settings().paper().is_some() && panel.get_paper_shown(), "on its paper");
    assert!(!settings().print.print_lift && !panel.get_print_lift());
    let count = |role: Role, label: &str| common::placed(w, role, label).len();
    assert_eq!((count(Role::Checkbox, "Print lift"), count(Role::Checkbox, "Colour compensation")), (1, 0));
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
