//! A digital photo on a black and white virtual film under the hand: the film lit in the stock menu shows the photo in
//! black and white with nothing written, an orthochromatic one printing a red darker than a panchromatic one; picked, it
//! is one step an Undo takes back to the colour film. Twin: the colour film's frame is no grey one.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Canvas, Menus, PanelState, PanelTab};
use oni_settings::VirtualStock;
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;
use crate::stock_hero::{drop_menu, point};

/// The canvas's frame as RGBA bytes.
fn frame(w: &oni_app::MainWindow) -> Vec<u8> {
    w.global::<Canvas>().get_frame().to_rgba8().expect("the canvas shows the photo").as_slice().iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect()
}

/// The frame's widest gap between a pixel's channels, in 8-bit codes.
fn chroma(frame: &[u8]) -> u8 {
    frame.chunks_exact(4).map(|p| p[0].abs_diff(p[1]).max(p[2].abs_diff(p[1]))).max().unwrap_or(0)
}

/// The frame's mean green code on its left half, the scene's red, over its right half, the scene's grey: each read
/// on its middle, away from the edge between them.
fn red_over_grey(frame: &[u8], width: usize) -> f32 {
    let height = frame.len() / 4 / width;
    let mean = |x0: usize, x1: usize| {
        let codes: Vec<f32> = (height / 3..2 * height / 3).flat_map(|y| (x0..x1).map(move |x| f32::from(frame[(y * width + x) * 4 + 1]))).collect();
        codes.iter().sum::<f32>() / codes.len() as f32
    };
    mean(width / 10, 4 * width / 10) / mean(6 * width / 10, 9 * width / 10)
}

pub(crate) fn a_black_and_white_film_lit_and_picked_shows_the_photo_in_black_and_white() {
    let root = common::Root::new("bw-film");
    std::fs::create_dir_all(root.join("Scans")).unwrap();
    // A red on the left half, a grey on the right.
    common::pixels_tiff(&root.join("Scans/A.tif"), 1200, 800, |x, _| if x < 600 { [0.75, 0.2, 0.15] } else { [0.4; 3] });
    let Some(app) = crate::film_effects::opened(root) else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (panel, menus) = (w.global::<PanelState>(), w.global::<Menus>());
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    rig.settle();
    let stored = || ctl.state.borrow().store().settings().clone();
    let shown = || ctl.state.borrow().shown_settings().cloned().expect("the canvas asked");
    let on = |s: &oni_settings::PipelineSettings| s.film().map(|f| f.stock);
    let width = || w.global::<Canvas>().get_frame().size().width as usize;
    // The check reads a film amid its menu's list: the photo is shot on Portra 800 first, as a pick does.
    let renders = ctl.state.borrow().renders();
    drop_menu(rig, w);
    w.global::<oni_app::Stocks>().invoke_picked(crate::stock_hero::stock(VirtualStock::of("kodak-portra-800").catalogue_id()));
    rig.key("\u{1b}");
    rig.until("the photo rendered on Portra 800", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    rig.advance(Duration::from_secs(5));
    rig.settle();
    let (own, undo) = (stored(), menus.get_live().undo);
    let colour = frame(w);
    assert_eq!(on(&own), Some(VirtualStock::of("kodak-portra-800")));
    assert!(chroma(&colour) > 20, "twin: the colour film's frame holds colour, {}", chroma(&colour));

    // The pointer on a black and white film shows the photo in black and white, nothing written.
    let lit = |stock: VirtualStock| {
        let renders = ctl.state.borrow().renders();
        point(rig, common::in_popup(w, Role::Button, stock.name()).middle());
        rig.until("the photo rendered on the film under the pointer", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
        rig.settle();
        assert_eq!(on(&shown()), Some(stock), "the canvas shows {stock:?}");
        assert!(shown().is_monochrome() && !stored().is_monochrome());
        frame(w)
    };
    drop_menu(rig, w);
    let pan = lit(VirtualStock::of("ilford-hp5-plus"));
    println!("shot {}", rig.shot("bw-film-hover-pan").display());
    let ortho = lit(VirtualStock::of("ilford-ortho-plus"));
    println!("shot {}", rig.shot("bw-film-hover-ortho").display());
    assert!(chroma(&pan) <= 2 && chroma(&ortho) <= 2, "a black and white frame: {} {}", chroma(&pan), chroma(&ortho));
    let (pan_red, ortho_red) = (red_over_grey(&pan, width()), red_over_grey(&ortho, width()));
    println!("the red over the grey: panchromatic {pan_red:.3}, orthochromatic {ortho_red:.3}");
    assert!(ortho_red < 0.9 * pan_red, "the orthochromatic film prints the red darker");
    assert_eq!((stored(), menus.get_live().undo), (own.clone(), undo), "nothing written, no step");
    rig.key("\u{1b}");
    rig.settle();
    assert_eq!(shown(), own, "Escape shows the colour film again");

    // Picked, the black and white film is written as one step; Undo takes the colour film back.
    drop_menu(rig, w);
    common::click(rig, w, common::in_popup(w, Role::Button, VirtualStock::of("ilford-hp5-plus").name()).middle());
    rig.settle();
    assert_eq!(stored(), own.on_stock(VirtualStock::of("ilford-hp5-plus")), "the pick writes the film lit");
    assert_eq!((panel.get_stock().name.as_str(), panel.get_stock().has_artwork), (VirtualStock::of("ilford-hp5-plus").name(), true));
    assert!(chroma(&frame(w)) <= 2 && menus.get_live().undo);
    println!("shot {}", rig.shot("bw-film-picked").display());
    let renders = ctl.state.borrow().renders();
    ctl.act(oni_app::Act::Undo);
    rig.until("the photo rendered on its colour film again", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    assert_eq!(stored(), own, "one Undo takes the colour film back, nothing of the black and white one kept");
    assert!(chroma(&frame(w)) > 20, "colour again");
}
