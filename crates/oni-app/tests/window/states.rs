//! The states read off the drawn window: out of reach far under an idle tab, whose label reads on its hollow, a
//! keycap keeping its relief there, a destructive glyph red only while reached, a lamp that reads at rest and lit
//! only while on, a switch's slot lit as a lamp while on, the hand's ring round a tile, whatever the photo, each
//! medal told by its figure without its colour.

use oni_app::StateProbe;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

use crate::common;

/// A flat key's height and its padding, `DS.Control.fieldHeight` and `DS.Space.md`.
const KEY: usize = 32;
const PAD: usize = 16;
/// A keycap's height, `DS.Control.keycapHeight`, and a square one's width.
const CAP: usize = 40;
/// The room a lamp takes after the word: its 6 pt and `DS.Space.xs`.
const LAMP_ROOM: usize = 10;
/// WCAG 1.4.11's floor for a control's state against what lies beside it.
const NON_TEXT: f64 = 3.0;
/// The luminance above which a lamp, or a switch's slot, reads lit.
const LIT: f64 = 0.5;
/// A switch's slot, `DS.Control.trackHeight`, centred in the knob's 26 pt row.
const SLOT_TOP: usize = 6;
const SLOT: usize = 14;
/// The knob's 20 pt and its travel, `DS.Control.toggleCellWidth`.
const KNOB: usize = 20;
const TRAVEL: usize = 24;
/// AA's 4.5:1 for small text, with the margin a quiet label keeps.
const TEXT: f64 = 5.0;
/// The pixels two figures must differ by, of the 64 inside a 16 pt medal's ring at one pixel a point.
const FIGURES_APART: usize = 6;

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
    // The idle label on the hollow beside it, left of its words: AA's 4.5:1 with a margin.
    let hollow = luminance(at(16 + 100 + 8, 16 + tabs_h / 2));
    eprintln!("idle label on its hollow {:.2}:1", contrast(idle, hollow));
    assert!(contrast(idle, hollow) >= TEXT, "an idle tab's label reads on its hollow: {:.2}:1", contrast(idle, hollow));
    // Twin: the out-of-reach word, far fainter, falls under it.
    assert!(contrast(off, hollow) < TEXT, "the measure sees a faint word: {:.2}:1", contrast(off, hollow));

    let (rest, reached, away, worn) = (red(16, 120), red(64, 120), red(112, 120), red(160, 120));
    eprintln!("red at rest {rest:.0}, reached {reached:.0}, out of reach {away:.0}, worn at rest {worn:.0}");
    assert!(reached > 80.0, "a destructive glyph turns red under the hand: {reached:.0}");
    assert!(rest < 30.0, "at rest it keeps the plain ink: {rest:.0}");
    assert!(away < 30.0, "out of reach it keeps the plain ink: {away:.0}");
    // Twin: the red worn at rest is seen.
    assert!(worn > 80.0, "the measure sees a red glyph: {worn:.0}");

    // A keycap's relief: its lit wall against its dark rim, the glyph's middle left out.
    let relief = |x0: usize| {
        let (lo, hi) = (120..120 + CAP)
            .flat_map(|y| (x0..x0 + CAP).map(move |x| (x, y)))
            .filter(|&(x, y)| !(x0 + 10..x0 + CAP - 10).contains(&x) || !(130..120 + CAP - 10).contains(&y))
            .map(|(x, y)| luminance(at(x, y)))
            .fold((f64::MAX, 0.0_f64), |(lo, hi), l| (lo.min(l), hi.max(l)));
        contrast(hi, lo)
    };
    let glyph = |x0: usize| peak(x0 + 12, 132, x0 + CAP - 12, 120 + CAP - 12);
    let (able, away, faded) = (relief(220), relief(270), relief(320));
    eprintln!("keycap relief {able:.2}:1, out of reach {away:.2}:1, faded whole {faded:.2}:1; glyph {:.3}, out of reach {:.3}", glyph(220), glyph(270));
    assert!((away - able).abs() < 0.1, "a keycap out of reach keeps its relief: {away:.2}:1, {able:.2}:1 when it can act");
    assert!(contrast(glyph(220), glyph(270)) >= 1.5, "its legend fades: {:.2}:1", contrast(glyph(220), glyph(270)));
    // Twin: a keycap faded whole keeps under half its relief and reads flat.
    assert!(faded - 1.0 < (able - 1.0) / 2.0, "the measure sees a cap faded whole: {faded:.2}:1 against {able:.2}:1");

    // The lamp's room after the word, on the key's middle rows.
    let lamp = |x: usize, key: f32| {
        let right = x + width(key) - PAD;
        peak(right - LAMP_ROOM, 168 + 4, right, 168 + KEY - 4)
    };
    let (dark, lit) = (lamp(16, probe.get_dark_width()), lamp(200, probe.get_lit_width()));
    // The face left of the word, and the twin's lamp of dark glass alone on the same face.
    let face = luminance(at(16 + 4, 168 + KEY / 2));
    let glass = peak(310 + 8, 168 + 8, 310 + 24, 168 + 24);
    eprintln!("lamp at rest {dark:.3}, lit {lit:.3}, face {face:.3}, dark glass {glass:.3}");
    assert!(contrast(dark, face) >= NON_TEXT, "a lamp at rest reads on the key's face: {:.2}:1", contrast(dark, face));
    assert!(lit > LIT, "a latching key on lights its lamp: {lit:.3}");
    assert!(dark < LIT, "a latching key off keeps its lamp unlit: {dark:.3}");
    // Twin: dark glass alone falls under the floor, a speck on the face.
    assert!(contrast(glass, face) < NON_TEXT, "the measure sees dark glass as a speck: {:.2}:1", contrast(glass, face));

    // The slot the knob leaves bare: on, the stretch at its left; off, the hollow at its right.
    let slot = |x: usize, from: usize, to: usize| peak(x + from, 20 + SLOT_TOP + 3, x + to, 20 + SLOT_TOP + SLOT - 3);
    let (switch_off, switch_on) = (slot(240, KNOB + 2, KNOB + TRAVEL - 2), slot(310, 3, TRAVEL - 3));
    let grey = peak(310 + 3, 81 + 3, 310 + TRAVEL - 3, 81 + SLOT - 3);
    eprintln!("switch off {switch_off:.3}, on {switch_on:.3}, a slider's grey stretch {grey:.3}");
    assert!(switch_on > LIT, "on, a switch's slot is lit as a lamp is: {switch_on:.3}");
    assert!(switch_off < LIT, "off, nothing in its hollow is lit: {switch_off:.3}");
    // Twin: the slider's grey stretch reads unlit.
    assert!(grey < LIT, "the measure sees a grey stretch as unlit: {grey:.3}");

    // The ground two to four points round a tile, at its left side's middle.
    let ground = luminance(at(10, 210 + 10));
    let (still, hand) = (luminance(at(24 - 3, 210 + 60)), luminance(at(200 - 3, 210 + 60)));
    eprintln!("ground {ground:.3}, round a tile at rest {still:.3}, under the hand {hand:.3}");
    assert!(contrast(hand, ground) >= NON_TEXT, "the hand's ring reads on the ground: {:.2}:1", contrast(hand, ground));
    // Twin: at rest nothing rings the tile.
    assert!(contrast(still, ground) < NON_TEXT, "the measure sees the bare ground: {:.2}:1", contrast(still, ground));

    // A medal's middle inside its ruled ring, its colour dropped: the pixels under half its face's light.
    let m = width(probe.get_medal());
    let shape = |i: usize| -> Vec<bool> {
        let (x0, y0) = (300 + i * m * 3 / 2 + m / 4, 260 + m / 4);
        let light: Vec<f64> = (y0..y0 + m / 2).flat_map(|y| (x0..x0 + m / 2).map(move |x| luminance(at(x, y)))).collect();
        let mut sorted = light.clone();
        sorted.sort_by(f64::total_cmp);
        let face = sorted[sorted.len() / 2];
        light.iter().map(|l| *l < face / 2.0).collect()
    };
    let apart = |a: &[bool], b: &[bool]| a.iter().zip(b).filter(|(a, b)| a != b).count();
    let (bronze, silver, gold, twin) = (shape(0), shape(1), shape(2), shape(3));
    let least = apart(&bronze, &silver).min(apart(&bronze, &gold)).min(apart(&silver, &gold));
    eprintln!("medals' figures at least {least} pixels apart, two golds {}", apart(&gold, &twin));
    assert!(least >= FIGURES_APART, "bronze, silver and gold differ by their figures alone: {least} pixels");
    // Twin: two golds read alike, so the measure counts the figure, never where it stands.
    assert!(apart(&gold, &twin) < FIGURES_APART, "the measure sees two like medals alike: {}", apart(&gold, &twin));
}

/// A readout's unit stands in a column of its own: one number bare, in percent and in stops ends its
/// digits at one edge, each unit past it, the widest within the column.
pub(crate) fn readouts_end_their_digits_at_one_edge() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = StateProbe::new().unwrap();
    probe.set_word("Value".into());
    let readouts = ["1.50", "1.50%", "1.50 EV"];
    probe.set_readouts(std::rc::Rc::new(slint::VecModel::from(readouts.map(slint::SharedString::from).to_vec())).into());
    oni_app::keycaps::serve(&probe);
    oni_app::slider_track::serve(&probe);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(400, 500));
    probe.show().unwrap();
    rig.settle();
    rig.shot("readouts");
    let placed = readouts.map(|text| common::the(&probe, i_slint_backend_testing::AccessibleRole::Text, text));
    // Where each readout's digits end: its left edge and the bare number's width.
    let digits = placed.map(|p| p.x + placed[0].width);
    let rights = placed.map(|p| p.x + p.width);
    eprintln!("digits end at {digits:?}, readouts at {rights:?}");
    // Within the pixel each edge rounds to.
    assert!(digits.iter().all(|d| (d - digits[0]).abs() <= 1.0), "the digits end at one edge: {digits:?}");
    assert!(rights[2] - digits[0] <= 19.0, "the column holds the widest unit: {:.1}", rights[2] - digits[0]);
    // Twin: right-aligned whole, the same readouts end their digits a unit's width apart.
    let whole = placed.map(|p| rights[0] - p.width + placed[0].width);
    assert!((whole[2] - whole[0]).abs() > 3.0, "the measure sees digits apart: {whole:?}");
}
