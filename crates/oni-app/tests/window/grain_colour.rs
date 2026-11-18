//! The grain's colour line in Film Effects through the window's pointer: an end dragged is one step to
//! undo, a double click brings it back, the block's Reset returns both ends; a film mode shows none.

use i_slint_backend_testing::AccessibleRole;
use oni_app::Store;
use oni_settings::blocks::GrainColour;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use std::time::Duration;

use crate::common;
use crate::film_effects::{at_full_size, opened, photo, press_rail};

const BLACK: &str = "Grain colour at the black point";
const WHITE: &str = "Grain colour at the white point";

/// The centre of the grab of the end named `label`, inward of the end.
fn end(w: &oni_app::MainWindow, label: &str) -> LogicalPosition {
    let p = common::the(w, AccessibleRole::Slider, label);
    LogicalPosition::new(p.x + p.width / 2.0, p.y + p.height / 2.0)
}

/// The end named `label` pressed, dragged `down` points in steps, then let go.
fn dragged(rig: &common::Rig, w: &oni_app::MainWindow, label: &str, down: f32) {
    let at = end(w, label);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
    for k in 1..=8 {
        let to = LogicalPosition::new(at.x, at.y + down * k as f32 / 8.0);
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: to });
        rig.pump();
    }
    let to = LogicalPosition::new(at.x, at.y + down);
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: to, button: PointerEventButton::Left });
    rig.settle();
}

fn clicked(rig: &common::Rig, at: LogicalPosition) {
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
    rig.pump();
}

pub(crate) fn the_grains_colour_line_moves_by_its_ends_and_comes_back() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the grain's colour line is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-grain-colour-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    photo(&root.join("Scans"));
    let (rig, shell) = opened(&gpu, &root);
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let grain = || ctl.state.borrow().store().settings().grain;
    let g = w.global::<Store>();
    assert_eq!(grain().colour, GrainColour::REST, "a photo opens on the colour's rest");
    let renders = ctl.state.borrow().renders();
    press_rail(&rig, w, "Grain", 0.8);
    rig.until("the grain rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    let laid = grain();

    // The black end sits two thirds up the plot; a drag down lowers it, and the frame renders again.
    let (black, white) = (end(w, BLACK), end(w, WHITE));
    assert!(black.y < white.y && black.x < white.x, "the black end stands left of and above the white: {black:?} {white:?}");
    let renders = ctl.state.borrow().renders();
    dragged(&rig, w, BLACK, 30.0);
    rig.until("the colour rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    let moved = grain().colour;
    println!("the black end at {} after the drag, its readout {}", moved.black, g.get_grain_colour_black_readout());
    assert!(moved.black < GrainColour::REST.black - 0.3 && moved.white == GrainColour::REST.white, "{moved:?}");
    assert!(end(w, BLACK).y > black.y + 20.0, "the dot follows the hand");
    println!("shot {}", rig.shot("grain-colour-fit").display());

    // One undo takes the whole drag back; twin: a second takes the grain itself.
    ctl.act(oni_app::Act::Undo);
    rig.settle();
    assert_eq!(grain(), laid, "one undo springs the whole drag back");
    ctl.act(oni_app::Act::Undo);
    rig.settle();
    assert_eq!(grain().amount, 0.0, "the step before the drag is the grain's own");
    ctl.act(oni_app::Act::Redo);
    rig.settle();
    assert_eq!(grain(), laid);

    // A double click on a moved end returns it to rest.
    dragged(&rig, w, WHITE, -40.0);
    assert!(grain().colour.white > 0.3, "the white end rose: {:?}", grain().colour);
    let at = end(w, WHITE);
    clicked(&rig, at);
    clicked(&rig, at);
    rig.settle();
    assert_eq!(grain().colour.white, GrainColour::REST.white, "a double click rests the end");

    // The block's Reset returns both ends with the grain.
    dragged(&rig, w, BLACK, 20.0);
    assert!(g.get_film_effects_modified() && grain().colour != GrainColour::REST);
    let block = oni_app::fields::Block::ALL.iter().position(|b| *b == oni_app::fields::Block::FilmEffects).unwrap();
    g.invoke_reset(block as i32);
    rig.settle();
    assert_eq!(grain().colour, GrainColour::REST, "the block's Reset rests the colour");
    assert!(!g.get_film_effects_modified(), "the block reads at rest");

    // At 100 %, where the grain's colour reads pixel by pixel.
    press_rail(&rig, w, "Grain", 0.8);
    rig.settle();
    at_full_size(&rig, &ctl);
    println!("shot {}", rig.shot("grain-colour-100").display());
    // A film mode lays no colour line.
    w.global::<oni_app::PanelState>().invoke_film_type_picked(0);
    rig.settle();
    assert!(common::placed(w, AccessibleRole::Slider, BLACK).is_empty(), "a negative lays no colour line");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
