//! A slider's relief through the window's pointer: at rest the rail's hollow and the knob's edge stay
//! light, under the hand and while it drags they deepen, and the hand gone they rest again.

use oni_app::KeyboardProbe;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize, Rgb8Pixel};
use std::path::PathBuf;

use crate::common;

/// The window's pixels to a point.
const SCALE: f32 = 2.0;

/// The window drawn in 8 bits a channel: 565's rounding, at each of the hollow's twelve faint strokes,
/// would erase a light shade and darken a deep one.
fn drawn(rig: &common::Rig) -> (usize, Vec<Rgb8Pixel>) {
    rig.window.request_redraw();
    let size = rig.window.size();
    let (w, h) = (size.width as usize, size.height as usize);
    let mut buffer = vec![Rgb8Pixel::default(); w * h];
    rig.window.draw_if_needed(|r| {
        r.render(&mut buffer, w);
    });
    (w, buffer)
}

fn light(p: Rgb8Pixel) -> i32 {
    p.r as i32 + p.g as i32 + p.b as i32
}

/// What the drawn window shows of a slider: its rail's hollow on the bare stretch, the share of the
/// ground its shaded top row loses; its knob's edge, the darkest point down the knob's face above the rail.
struct Relief {
    hollow: f32,
    edge: i32,
    beside: i32,
}

fn relief(rig: &common::Rig, p: &KeyboardProbe) -> Relief {
    let (w, px) = drawn(rig);
    let at = |x: f32, y: f32| light(px[(y * SCALE) as usize * w + (x * SCALE) as usize]);
    let (x, y) = (p.get_knob_x(), p.get_knob_y());
    // The 8 pt rail is centred on the knob's row: its top row lies under every stroke of the shade, its
    // lower middle under none.
    let top = y - 4.0;
    let bare = x + 60.0;
    let (shaded, ground) = (at(bare, top + 1.0), at(bare, top + 5.5));
    let edge = (0..22).map(|i| at(x + 4.0, y - 16.0 + i as f32 / SCALE)).min().unwrap();
    Relief { hollow: 1.0 - shaded as f32 / ground as f32, edge, beside: at(bare, y - 9.0) }
}

fn probe() -> (common::Rig, KeyboardProbe) {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = KeyboardProbe::new().unwrap();
    oni_app::keycaps::serve(&probe);
    oni_app::slider_track::serve(&probe);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: SCALE });
    rig.window.set_size(PhysicalSize::new(400 * SCALE as u32, 400 * SCALE as u32));
    probe.show().unwrap();
    rig.drawn();
    (rig, probe)
}

pub(crate) fn a_sliders_relief_is_light_at_rest_and_deepens_under_the_hand() {
    let (rig, p) = probe();
    let to = |x: f32, y: f32| {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
        rig.settle();
    };
    let (knob, below) = (LogicalPosition::new(p.get_knob_x(), p.get_knob_y()), p.get_knob_y() + 40.0);
    to(p.get_value_x(), p.get_bare_y());
    let rest = relief(&rig, &p);
    rig.shot("slider-relief-rest");

    // Under the hand the hollow reaches a well's whole shade, 0.25, and the knob's edge is black. The
    // software renderer truncates each of the twelve strokes it blends: a deep shade reads up to 0.3.
    to(knob.x, knob.y);
    let hand = relief(&rig, &p);
    rig.shot("slider-relief-hand");
    println!("rest {:.3} {} {}, hand {:.3} {} {}", rest.hollow, rest.edge, rest.beside, hand.hollow, hand.edge, hand.beside);
    assert!((0.2..0.32).contains(&hand.hollow), "the hand's hollow is a well's: {:.3}", hand.hollow);
    assert!(hand.edge < 24, "the hand's knob is edged in black: {}", hand.edge);
    // At rest, about half of each: a hollow still milled, and an edge half black over the cap's walls.
    let calm = |hollow: f32| hollow > 0.05 && hollow < 0.6 * hand.hollow;
    assert!(calm(rest.hollow), "the rest's hollow is about half the hand's: {:.3} against {:.3}", rest.hollow, hand.hollow);
    assert!(rest.edge > hand.edge + 40, "the knob's edge is lighter at rest: {} against {}", rest.edge, hand.edge);
    // Twin: the hand's whole well passed off as the rest's is caught.
    assert!(!calm(hand.hollow), "a whole well at rest would be seen: {:.3}", hand.hollow);
    // Twin: the panel beside the rail stays as it was, so what moved is the slider's own relief.
    assert_eq!(rest.beside, hand.beside, "the panel beside the slider never deepens");

    // Held, the knob dragged off its row keeps the depth; let go, the slider rests again.
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: knob, button: PointerEventButton::Left });
    to(knob.x, below);
    let held = relief(&rig, &p);
    assert!((held.hollow - hand.hollow).abs() < 0.02 && held.edge == hand.edge, "a drag keeps the depth: {:.3} {}", held.hollow, held.edge);
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: LogicalPosition::new(knob.x, below), button: PointerEventButton::Left });
    to(p.get_value_x(), p.get_bare_y());
    let after = relief(&rig, &p);
    assert!((after.hollow - rest.hollow).abs() < 0.02 && after.edge == rest.edge, "the hand gone, it rests: {:.3} {}", after.hollow, after.edge);
}
