//! A digital photo opens shot on the neutral film, the Color tab's Film block showing its stock and the light it keeps
//! the scene at: Shooting exposure moves the film alone and renders the photo again; the settings key shows the reference its
//! negative is read on, which no hand writes. Twin: a negative shows no Film block.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Canvas, PanelState, PanelTab, Store};
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;
use crate::film_effects::{opened, press_rail};

pub(crate) fn a_digital_photo_opens_on_the_neutral_film_and_shows_its_reference() {
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
    assert_eq!((film.stock, film.exposure), (oni_settings::VirtualStock::NEUTRAL, 0.0));
    assert_eq!(panel.get_stock().name.as_str(), "Neutral");
    assert!(shown(Role::Text, "Film") && shown(Role::Button, "Select Film Stock") && !shown(Role::Tab, "None"), "its stock and its key, no film None");
    // The neutral film keeps the scene's own light: no seal beside its name.
    assert!(panel.get_film_balance() == 0 && !shown(Role::Text, "Daylight") && !shown(Role::Text, "Tungsten"), "a seal on the neutral film");
    assert!(shown(Role::Slider, "Shooting exposure") && !w.global::<Store>().get_film_modified(), "its exposure at rest");
    let on = frame();
    println!("shot {}", rig.shot("virtual-film-on").display());

    rendered("the film's exposure rendered", &|| press_rail(rig, w, "Shooting exposure", 0.75));
    let exposed = settings();
    // Thirds of a stop from two under to two over: three quarters of the rail is the tenth notch of thirteen, a stop over.
    assert_eq!(exposed.film().unwrap().exposure, 1.0, "a press at three quarters lays its notch: {:?}", exposed.film());
    let reference = oni_settings::VirtualStock::NEUTRAL.reference();
    assert_eq!((exposed.gains.stops, exposed.levels), (reference.stops, reference.levels()), "the exposure alone moved");
    assert!(w.global::<Store>().get_film_modified(), "the block's Reset shows");
    assert_ne!(frame(), on);

    // No settings key: the film's reference is the rest the Source tab's Filtration and Per-Channel Levels show.
    assert!(!shown(Role::Button, "Show the film's reference"), "the Film block's settings key");
    // Twin: a film mode shows no Film block.
    panel.invoke_film_type_picked(0);
    rig.settle();
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    rig.settle();
    assert!(!shown(Role::Slider, "Shooting exposure"), "a negative shoots on no virtual film");
}
