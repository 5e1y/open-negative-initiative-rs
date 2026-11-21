//! A digital photo shot on Portra 800 through the Color tab's Film block: the tab picks the film, which
//! lays its stock's reference and renders the photo again as that negative; Film exposure moves the
//! film alone; the settings key shows the negative's gains and ends; None gives the photo's settings back.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Canvas, PanelState, PanelTab, Store};
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;
use crate::film_effects::{opened, press_rail};

pub(crate) fn a_digital_photo_is_shot_on_portra_and_back() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the Film block is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-virtual-film-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Scans")).unwrap();
    common::graded_tiff(&root.join("Scans/A.tif"), 2400, 1600, 0.7);
    let (rig, shell) = opened(&gpu, &root);
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let panel = w.global::<PanelState>();
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    rig.settle();
    let settings = || ctl.state.borrow().store().settings().clone();
    let shown = |role: Role, name: &str| !common::placed(w, role, name).is_empty();
    let frame = || w.global::<Canvas>().get_frame().to_rgba8().expect("the canvas shows the photo").as_slice().to_vec();
    let rendered = |what: &str, act: &dyn Fn()| {
        let renders = ctl.state.borrow().renders();
        act();
        rig.until(what, Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
        rig.settle();
    };
    let photo = (settings(), frame());
    assert!(shown(Role::Text, "Film") && shown(Role::Tab, "Portra 800"), "a digital photo's Film block");
    assert!(!shown(Role::Slider, "Film exposure"), "no film, no exposure to give it");

    rendered("the film rendered", &|| common::click(&rig, w, common::the(w, Role::Tab, "Portra 800").middle()));
    let film = settings().virtual_film.expect("the tab laid a film");
    assert_eq!(film, oni_pipeline::film::stock(oni_settings::VirtualStock::KodakPortra800).reference, "on its stock's own reference");
    assert!(shown(Role::Slider, "Film exposure") && !shown(Role::Slider, "Red ↔ Cyan"), "its exposure shown, its reference behind the key");
    let on = frame();
    assert_ne!(on, photo.1, "the photo renders again as the negative");
    assert!(!w.global::<Store>().get_film_modified(), "the film opens on its rest");
    println!("shot {}", rig.shot("virtual-film-on").display());

    rendered("the film's exposure rendered", &|| press_rail(&rig, w, "Film exposure", 0.75));
    let exposed = settings().virtual_film.unwrap();
    assert!((exposed.exposure - 2.0).abs() < 0.25, "a press at three quarters gives about two stops: {exposed:?}");
    assert_eq!((exposed.stops, exposed.black, exposed.white), (film.stops, film.black, film.white), "the exposure alone moved");
    assert!(w.global::<Store>().get_film_modified(), "the block's Reset shows");
    assert_ne!(frame(), on);

    common::click(&rig, w, common::the(w, Role::Button, "Show the negative's gains and ends").middle());
    rig.settle();
    for name in ["Red ↔ Cyan", "Green ↔ Magenta", "Blue ↔ Yellow", "Red black", "Red white"] {
        assert!(shown(Role::Slider, name), "{name} behind the settings key");
    }
    println!("shot {}", rig.shot("virtual-film-reference").display());

    rendered("the photo back", &|| common::click(&rig, w, common::the(w, Role::Tab, "None").middle()));
    assert_eq!(settings(), photo.0, "None gives the photo's settings back");
    let gap = |a: &[slint::Rgba8Pixel], b: &[slint::Rgba8Pixel]| a.iter().zip(b).map(|(p, q)| [p.r.abs_diff(q.r), p.g.abs_diff(q.g), p.b.abs_diff(q.b)].into_iter().max().unwrap()).max().unwrap();
    let back = frame();
    let (back, film_on) = (gap(&back, &photo.1), gap(&on, &photo.1));
    println!("the photo back against the photo: {back} codes at worst; the film on: {film_on}");
    // The render itself is the photo's to the bit (the pipeline's check); the canvas's first landing
    // after the open may differ by a code or two from a later one.
    assert!(back <= 2 && film_on > 50, "{back} {film_on}");
    // Twin: a film mode shows no Film block.
    panel.invoke_film_type_picked(0);
    rig.settle();
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    rig.settle();
    assert!(!shown(Role::Tab, "Portra 800"), "a negative shoots on no virtual film");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
