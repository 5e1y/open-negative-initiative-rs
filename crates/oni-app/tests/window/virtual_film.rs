//! A digital photo opens shot on Portra 800, the Color tab's Film block showing its stock and the white it lights the
//! scene at: Film exposure moves the film alone and renders the photo again; the settings key shows the reference its
//! negative is read on, which no hand writes. Twin: a negative shows no Film block.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Canvas, PanelState, PanelTab, Store};
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;
use crate::film_effects::{opened, press_rail};

pub(crate) fn a_digital_photo_opens_on_portra_and_shows_its_reference() {
    let root = common::Root::new("virtual-film");
    std::fs::create_dir_all(root.join("Scans")).unwrap();
    common::graded_tiff(&root.join("Scans/A.tif"), 2400, 1600, 0.7);
    let Some(app) = opened(root) else { return };
    let common::App { rig, shell, .. } = &app;
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
    let film = settings().film().expect("a digital photo is shot on its film");
    assert_eq!((film.stock, film.exposure), (oni_settings::VirtualStock::of("kodak-portra-800"), 0.0));
    assert_eq!(panel.get_stock().name.as_str(), "Kodak Portra 800");
    assert!(shown(Role::Text, "Film") && shown(Role::Button, "Select Film Stock") && !shown(Role::Tab, "None"), "its stock and its key, no film None");
    assert!(shown(Role::Text, &panel.get_film_white()) && panel.get_film_white().starts_with("5500 K"), "the stock's white in sight: {}", panel.get_film_white());
    assert!(shown(Role::Slider, "Film exposure") && !w.global::<Store>().get_film_modified(), "its exposure at rest");
    let on = frame();
    println!("shot {}", rig.shot("virtual-film-on").display());

    rendered("the film's exposure rendered", &|| press_rail(rig, w, "Film exposure", 0.75));
    let exposed = settings();
    assert!((exposed.film().unwrap().exposure - 2.0).abs() < 0.25, "a press at three quarters gives about two stops: {:?}", exposed.film());
    let reference = oni_settings::VirtualStock::of("kodak-portra-800").reference();
    assert_eq!((exposed.gains.stops, exposed.levels), (reference.stops, reference.levels()), "the exposure alone moved");
    assert!(w.global::<Store>().get_film_modified(), "the block's Reset shows");
    assert_ne!(frame(), on);

    assert!(!shown(Role::Text, "Gains"), "the reference behind the settings key");
    common::click(rig, w, common::the(w, Role::Button, "Show the film's reference").middle());
    rig.settle();
    assert!(shown(Role::Text, "Gains") && shown(Role::Text, "Black"), "the reference behind the settings key");
    println!("shot {}", rig.shot("virtual-film-reference").display());
    // Twin: a film mode shows no Film block.
    panel.invoke_film_type_picked(0);
    rig.settle();
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    rig.settle();
    assert!(!shown(Role::Slider, "Film exposure"), "a negative shoots on no virtual film");
}
