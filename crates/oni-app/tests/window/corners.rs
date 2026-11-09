//! The canvas card is round at every corner but its top right, square against the panel's readout, and
//! those a folded column squares; the readout's glass, the scanner's screen too, runs edge to edge from the
//! card's top, square.

use oni_app::{Act, Bar, DS};
use slint::{Color, ComponentHandle};

use crate::common;
use crate::panel_bindings::opened;

/// Where the canvas tops, under a check's menu bar and `ShellStyle.bar-height`; the library's width; the
/// plots' round.
const BAR: f32 = 40.0 + 64.0;
const LIBRARY: f32 = 260.0;
const ROUND: f32 = 6.0;

fn rgb565(c: Color) -> u16 {
    ((u16::from(c.red()) >> 3) << 11) | ((u16::from(c.green()) >> 2) << 5) | (u16::from(c.blue()) >> 3)
}

fn pixel(rig: &common::Rig, x: f32, y: f32) -> u16 {
    let (width, buffer) = rig.drawn();
    buffer[y as usize * width + x as usize].0
}

/// Whether a pixel `k` points in from a square's corner, along both sides, lies inside a round of `r`.
fn inside(r: f32, k: f32) -> bool {
    (r - k - 0.5) * std::f32::consts::SQRT_2 <= r
}

pub(crate) fn the_canvas_squares_its_corner_against_a_readout_run_edge_to_edge() {
    let Some((rig, shell, root)) = opened("corners") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let ds = w.global::<DS>();
    let (ground, canvas) = (rgb565(ds.get_palette_panel()), rgb565(ds.get_palette_canvas()));
    rig.settle();
    let panel = w.get_panel_x();

    // The canvas's top right corner is square against the readout; its top left keeps its round.
    assert_eq!(pixel(&rig, panel - 1.0, BAR), canvas, "the top right corner is square");
    assert_eq!(pixel(&rig, LIBRARY, BAR), ground, "twin: the top left corner lies outside its round");
    assert_eq!(pixel(&rig, LIBRARY + 5.0, BAR + 5.0), canvas, "the canvas within it");

    // The readout's glass from the card's top, edge to edge: its very corners are the glass's; twin: the
    // plots' round would leave the corner pixel to the ground.
    assert_ne!(pixel(&rig, panel, BAR), ground, "the readout's corner beside the canvas is the glass's");
    assert_ne!(pixel(&rig, 1279.0, BAR), ground, "the readout runs to the window's edge");
    assert!(!inside(ROUND, 0.0) && inside(ROUND, 2.0), "a round would take the corner pixel");

    // The scanner's screen in the readout's place, as square.
    w.global::<Bar>().invoke_act(Act::FaceScanner);
    rig.settle();
    assert_ne!(pixel(&rig, panel, BAR), ground, "the screen's glass at its very corner");
    w.global::<Bar>().invoke_act(Act::FacePro);
    rig.settle();

    // Twin: the panel folded squares the canvas's right side, and the same probe sees it.
    ctl.act(Act::HidePanel);
    rig.settle();
    assert_eq!(pixel(&rig, 1279.0, BAR), canvas, "a folded column's side is square");
    let _ = std::fs::remove_dir_all(&root);
}
