//! The states read off the drawn window: out of reach far under an idle tab, a destructive glyph red only
//! while reached, a latch's lamp lit only while on, the hand's ring round a tile on the ground, whatever the photo.

use oni_app::StateProbe;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

use crate::common;

/// A flat key's height and its padding, `DS.Control.fieldHeight` and `DS.Space.md`.
const KEY: usize = 32;
const PAD: usize = 16;
/// The room a lamp takes after the word: its 6 pt and `DS.Space.xs`.
const LAMP_ROOM: usize = 10;
/// WCAG 1.4.11's floor for a control's state against what lies beside it.
const NON_TEXT: f64 = 3.0;

/// An sRGB channel of 0 to 255, linearised.
fn linear(c: f64) -> f64 {
    let c = c / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn rgb(p: u16) -> (f64, f64, f64) {
    let (r, g, b) = ((p >> 11) & 0x1f, (p >> 5) & 0x3f, p & 0x1f);
    (f64::from(r) * 255.0 / 31.0, f64::from(g) * 255.0 / 63.0, f64::from(b) * 255.0 / 31.0)
}

fn luminance(p: u16) -> f64 {
    let (r, g, b) = rgb(p);
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

fn contrast(a: f64, b: f64) -> f64 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

pub(crate) fn every_state_reads_apart_from_its_neighbour() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = StateProbe::new().unwrap();
    probe.set_word("Scanner".into());
    oni_app::keycaps::serve(&probe);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(400, 340));
    probe.show().unwrap();
    rig.settle();
    rig.shot("states");
    let (w, px) = rig.drawn();
    let px: Vec<u16> = px.into_iter().map(|p| p.0).collect();
    let at = |x: usize, y: usize| px[y * w + x];
    // The brightest pixel of a box: a word's or a lamp's ink at its core.
    let peak = |x0: usize, y0: usize, x1: usize, y1: usize| {
        (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| luminance(at(x, y))).fold(0.0, f64::max)
    };
    // How far a box's reddest pixel leans to red, 0 to 255.
    let red = |x0: usize, y0: usize| {
        (y0 + 4..y0 + KEY - 4)
            .flat_map(|y| (x0 + 4..x0 + KEY - 4).map(move |x| (x, y)))
            .map(|(x, y)| {
                let (r, g, b) = rgb(at(x, y));
                r - (g + b) / 2.0
            })
            .fold(0.0, f64::max)
    };
    let width = |l: f32| l.round() as usize;

    // The idle tab's label, clear of the pill on the left half and of the hollow's rim.
    let tabs_h = width(probe.get_tabs_height());
    let idle = peak(16 + 100 + 8, 16 + 8, 16 + 200 - 8, 16 + tabs_h - 8);
    let off = peak(16 + 4, 72 + 4, 16 + width(probe.get_off_width()) - 4, 72 + KEY - 4);
    let was = peak(200 + 4, 72 + 4, 200 + width(probe.get_was_width()) - 4, 72 + KEY - 4);
    eprintln!("idle {idle:.3}, out of reach {off:.3}, at the missing depth {was:.3}");
    assert!(contrast(idle, off) >= 1.5, "a key out of reach reads apart from an idle tab: {:.2}:1", contrast(idle, off));
    // Twin: at a missing photo's fade, the key's word stands as bright as the idle label.
    assert!(contrast(idle, was) < 1.5, "the measure sees two inks alike: {:.2}:1", contrast(idle, was));

    let (rest, reached, away, worn) = (red(16, 120), red(64, 120), red(112, 120), red(160, 120));
    eprintln!("red at rest {rest:.0}, reached {reached:.0}, out of reach {away:.0}, worn at rest {worn:.0}");
    assert!(reached > 80.0, "a destructive glyph turns red under the hand: {reached:.0}");
    assert!(rest < 30.0, "at rest it keeps the plain ink: {rest:.0}");
    assert!(away < 30.0, "out of reach it keeps the plain ink: {away:.0}");
    // Twin: the red worn at rest is seen.
    assert!(worn > 80.0, "the measure sees a red glyph: {worn:.0}");

    // The lamp's room after the word, on the key's middle rows.
    let lamp = |x: usize, key: f32| {
        let right = x + width(key) - PAD;
        peak(right - LAMP_ROOM, 168 + 4, right, 168 + KEY - 4)
    };
    let (dark, lit) = (lamp(16, probe.get_dark_width()), lamp(200, probe.get_lit_width()));
    eprintln!("lamp dark {dark:.3}, lit {lit:.3}");
    assert!(lit > 0.5, "a latching key on lights its lamp: {lit:.3}");
    // Twin: off, the lamp stays dark glass on the face.
    assert!(dark < 0.1, "a latching key off keeps its lamp dark: {dark:.3}");

    // The ground two to four points round a tile, at its left side's middle.
    let ground = luminance(at(10, 210 + 10));
    let (still, hand) = (luminance(at(24 - 3, 210 + 60)), luminance(at(200 - 3, 210 + 60)));
    eprintln!("ground {ground:.3}, round a tile at rest {still:.3}, under the hand {hand:.3}");
    assert!(contrast(hand, ground) >= NON_TEXT, "the hand's ring reads on the ground: {:.2}:1", contrast(hand, ground));
    // Twin: at rest nothing rings the tile.
    assert!(contrast(still, ground) < NON_TEXT, "the measure sees the bare ground: {:.2}:1", contrast(still, ground));
}
