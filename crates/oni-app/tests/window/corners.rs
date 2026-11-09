//! The canvas card is round at every corner but those a folded column squares; the readout's glass, the
//! scanner's screen too, takes the plots' lower round.

use oni_app::{Act, Bar, DS};
use slint::{Color, ComponentHandle};

use crate::common;
use crate::panel_bindings::opened;

/// Where the canvas tops, under a check's menu bar and `ShellStyle.bar-height`, the readout's inset,
/// `Space.m`, and the plots' round.
const BAR: f32 = 40.0 + 64.0;
const INSET: f32 = 16.0;
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

pub(crate) fn the_canvas_keeps_its_round_and_the_readout_a_lower_one() {
    let Some((rig, shell, root)) = opened("corners") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let ds = w.global::<DS>();
    let (ground, canvas) = (rgb565(ds.get_palette_panel()), rgb565(ds.get_palette_canvas()));
    rig.settle();
    let panel = w.get_panel_x();

    // The canvas's top right corner shows the ground outside its round, and the canvas within it.
    assert_eq!(pixel(&rig, panel - 1.0, BAR), ground, "the corner itself lies outside the round");
    assert_eq!(pixel(&rig, panel - 5.0, BAR + 5.0), canvas, "the canvas within the round");

    // The readout's glass: its corner the ground's, two points in its own; twin: the former round leaves that out.
    let (x, y) = (panel + INSET, BAR + INSET);
    assert_eq!(pixel(&rig, x, y), ground, "the readout's corner lies outside its round");
    assert_ne!(pixel(&rig, x + 2.0, y + 2.0), ground, "two points in stands the glass");
    assert!(inside(ROUND, 2.0) && !inside(ROUND, 0.0) && !inside(12.0, 2.0), "twin: a 12 pt round leaves that pixel out");

    // The scanner's screen in the readout's place, at the same round.
    w.global::<Bar>().invoke_act(Act::FaceScanner);
    rig.settle();
    assert_ne!(pixel(&rig, x + 2.0, y + 2.0), ground, "two points in stands the screen's glass");
    w.global::<Bar>().invoke_act(Act::FacePro);
    rig.settle();

    // Twin: the panel folded squares the canvas's right side, and the same probe sees it.
    ctl.act(Act::HidePanel);
    rig.settle();
    assert_eq!(pixel(&rig, 1279.0, BAR), canvas, "a folded column's side is square");
    let _ = std::fs::remove_dir_all(&root);
}
