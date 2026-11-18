//! The Levels block of a digital photo's Colour tab through the window's pointer: its five tabs pick RGB,
//! Luma, R, G and B, a drag of each white writes that window alone and renders, a double click puts it
//! back, the block's Reset rests all five. Twin: a negative lays no such block, its Roll tab three tabs.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::editors::{self, Plot};
use oni_app::{Editors, PanelState, PanelTab};
use oni_settings::levels::{Levels, LevelsChannel};
use slint::{ComponentHandle, Model};
use std::time::Duration;

use crate::common;
use crate::editors::{drag, press, release};

const TABS: [&str; 5] = ["RGB", "Luma", "R", "G", "B"];

/// The histogram's mean place on its bins, per channel, of the picture last rendered.
fn means(ctl: &oni_app::app::Ctl) -> [f32; 3] {
    let state = ctl.state.borrow();
    let h = state.histogram().expect("a histogram of the render");
    std::array::from_fn(|c| {
        let counts = h.channel(c);
        let total: f32 = counts.iter().sum();
        counts.iter().enumerate().map(|(i, n)| i as f32 * n).sum::<f32>() / total.max(1e-6)
    })
}

pub(crate) fn a_digital_photos_levels_move_each_mode_through_the_real_editor() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the Levels block is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-digital-levels-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    common::graded_tiff(&root.join("Scans/A.tif"), 600, 400, 0.7);
    let (rig, shell) = crate::film_effects::opened(&gpu, &root);
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (panel, g) = (w.global::<PanelState>(), w.global::<Editors>());
    // Tall enough for the Colour tab to stand whole, Levels under Film Effects.
    rig.window.set_size(slint::PhysicalSize::new(1280, 2400));
    panel.invoke_film_type_picked(0);
    rig.settle();
    let tab = |t: PanelTab| {
        panel.set_tab(t);
        panel.invoke_tab_picked(t);
        rig.settle();
    };
    let settings = || ctl.state.borrow().store().settings().clone();
    let title = || common::placed(w, Role::Text, "Levels");

    // Twin first: a negative's Colour tab lays no Levels, its Roll tab's per-channel block three tabs.
    tab(PanelTab::Color);
    assert!(title().is_empty(), "a negative lays no Levels on its Colour tab");
    tab(PanelTab::Roll);
    g.set_levels_folded(false);
    rig.settle();
    assert_eq!(g.get_levels_marked().row_count(), 3, "R, G and B on a film");
    assert!(common::placed(w, Role::Tab, "Luma").is_empty() && common::placed(w, Role::Tab, "RGB").is_empty());

    panel.invoke_film_type_picked(3);
    rig.settle();
    assert_eq!(settings().mode, oni_settings::ConversionMode::Digital);
    let film = settings().levels;
    tab(PanelTab::Color);
    assert_eq!(title().len(), 1, "a digital photo lays Levels on its Colour tab");
    assert!(g.get_picture_levels_folded(), "it opens folded");
    common::click(&rig, w, title()[0].middle());
    rig.settle();
    assert!(!g.get_picture_levels_folded(), "its title unfolds it");
    assert_eq!(g.get_levels_marked().row_count(), 5);
    println!("shot {}", rig.shot("digital-levels-open").display());
    rig.until("a histogram", Duration::from_secs(60), || ctl.state.borrow().histogram().is_some());

    let size = editors::size(&ctl, Plot::Levels);
    assert!(size.w > 200.0, "the track is laid out: {size:?}");
    for (i, name) in TABS.iter().enumerate() {
        let tabs = common::placed(w, Role::Tab, name);
        assert_eq!(tabs.len(), 1, "one {name} tab shown, the Curves folded");
        common::click(&rig, w, tabs[0].middle());
        rig.settle();
        assert_eq!(g.get_levels_channel(), i as i32, "{name} picked");
        let channel = LevelsChannel::PICTURE[i];
        let before = (settings().picture_levels, means(&ctl));
        let knob = g.get_levels_knobs().row_data(4).unwrap();
        assert_eq!(knob.label.as_str(), "White");
        assert!((f64::from(knob.x) - size.w).abs() < 1e-3, "{name}'s white rests at the track's end: {}", knob.x);
        let origin = editors::origin(&ctl, Plot::Levels);
        let grab = (origin.0 + f64::from(knob.x) - 4.0, origin.1 + size.h / 2.0);
        let renders = ctl.state.borrow().renders();
        press(&rig, grab);
        drag(&rig, grab, (-60.0, 0.0), 20);
        release(&rig, (grab.0 - 60.0, grab.1));
        rig.until("the levels rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
        rig.settle();
        let after = settings().picture_levels;
        let white = after.get(channel).white;
        assert!((f64::from(1.0 - white) * size.w - 60.0).abs() < 0.5, "{name}'s white moved by the pointer's travel: {white}");
        for other in LevelsChannel::PICTURE.into_iter().filter(|c| *c != channel) {
            assert_eq!(after.get(other), before.0.get(other), "{name} moved {other:?}");
        }
        assert_eq!(settings().levels, film, "the film's windows stay");
        assert!(g.get_levels_marked().row_data(i).unwrap() && g.get_levels_modified(), "{name}'s tab lit, the block's Reset shown");
        if channel == LevelsChannel::Red {
            let moved = [0, 1, 2].map(|c| means(&ctl)[c] - before.1[c]);
            println!("R's white moves the histogram's means by {moved:?} bins");
            assert!(moved[0] > 1.0 && moved[1].abs() < moved[0] / 4.0 && moved[2].abs() < moved[0] / 4.0, "R lifted red alone: {moved:?}");
        }
        // A double click on the white puts it back on the picture's end.
        let x = origin.0 + f64::from(g.get_levels_knobs().row_data(4).unwrap().x);
        std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
        for _ in 0..2 {
            press(&rig, (x, grab.1));
            release(&rig, (x, grab.1));
        }
        rig.settle();
        assert_eq!(settings().picture_levels.get(channel).white, 1.0, "{name}'s double click");
        // Moved again for the block's Reset below.
        std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
        press(&rig, grab);
        drag(&rig, grab, (-30.0, 0.0), 10);
        release(&rig, (grab.0 - 30.0, grab.1));
        rig.settle();
        std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    }
    println!("shot {}", rig.shot("digital-levels-moved").display());
    assert!(LevelsChannel::PICTURE.iter().all(|c| settings().picture_levels.get(*c) != Levels::default()), "all five moved");
    let reset: Vec<_> = common::placed(w, Role::Button, "Reset").into_iter().filter(|k| title()[0].level_with(k)).collect();
    assert_eq!(reset.len(), 1, "one Reset on the row of Levels");
    common::click(&rig, w, reset[0].middle());
    rig.settle();
    assert!(settings().picture_levels.is_neutral(), "the Reset rests all five");
    assert!(!g.get_levels_modified());
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
