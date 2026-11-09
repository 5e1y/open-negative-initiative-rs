//! A rail of hues through the window's pointer: at rest a faint hue at each end over the grey ground,
//! its full gradient under the hand and while its keys step it, the step keys grey, a grey rail whole.

use oni_app::HueProbe;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize, Rgb8Pixel};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// The window's pixels to a point.
const SCALE: f32 = 2.0;
/// How far in from a rail's end its colour is read: inside its round, past the hollow's lit lip.
const IN: f32 = 5.0;

/// The window drawn in 8 bits a channel, so a faint hue is not rounded away.
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

/// A rail's two ends and its stretch a third in, as drawn: what the eye reads of its hues.
#[derive(Debug)]
struct Ends {
    low: Rgb8Pixel,
    high: Rgb8Pixel,
    between: Rgb8Pixel,
}

fn chroma(p: Rgb8Pixel) -> i32 {
    let (r, g, b) = (p.r as i32, p.g as i32, p.b as i32);
    r.max(g).max(b) - r.min(g).min(b)
}

fn light(p: Rgb8Pixel) -> i32 {
    p.r as i32 + p.g as i32 + p.b as i32
}

fn at(rig: &common::Rig, x: f32, y: f32) -> Rgb8Pixel {
    let (w, px) = drawn(rig);
    px[(y * SCALE) as usize * w + (x * SCALE) as usize]
}

fn ends(rig: &common::Rig, from: f32, to: f32, y: f32) -> Ends {
    Ends { low: at(rig, from + IN, y), high: at(rig, to - IN, y), between: at(rig, from + 0.4 * (to - from), y) }
}

/// The most colour down a key's middle, over its face and its legend.
fn key_chroma(rig: &common::Rig, x: f32, y: f32) -> i32 {
    (-10..=10).map(|i| chroma(at(rig, x, y + i as f32))).max().unwrap()
}

fn probe() -> (common::Rig, HueProbe) {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let probe = HueProbe::new().unwrap();
    oni_app::keycaps::serve(&probe);
    oni_app::slider_track::serve(&probe);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: SCALE });
    rig.window.set_size(PhysicalSize::new(400 * SCALE as u32, 400 * SCALE as u32));
    probe.show().unwrap();
    rig.settle();
    (rig, probe)
}

pub(crate) fn a_rail_of_hues_is_faint_at_rest_and_whole_under_the_hand_or_its_keys() {
    let (rig, p) = probe();
    let to = |x: f32, y: f32| {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
        rig.settle();
    };
    let temperature = || ends(&rig, p.get_from(), p.get_to(), p.get_temperature_y());
    let cyan = || ends(&rig, p.get_keyed_from(), p.get_keyed_to(), p.get_c_y());
    let dark = || ends(&rig, p.get_keyed_from(), p.get_keyed_to(), p.get_d_y());
    to(p.get_to() + 40.0, 380.0);
    let (rest, c_rest, d_rest) = (temperature(), cyan(), dark());
    println!("shot {}", rig.shot("slider-hues-rest").display());

    // Under the hand Temperature shows its blue and its yellow whole.
    to(p.get_from() + 0.25 * (p.get_to() - p.get_from()), p.get_temperature_y());
    let hand = temperature();
    println!("shot {}", rig.shot("slider-hues-hand").display());
    println!("rest {rest:?}\nhand {hand:?}");
    assert!(chroma(hand.low) > 150 && chroma(hand.high) > 150, "under the hand the hues are whole: {hand:?}");
    // At rest a faint hue at each end, still on its side: blue at the cool end, yellow at the warm one.
    let faint = |e: &Ends| (8..60).contains(&chroma(e.low)) && (8..60).contains(&chroma(e.high));
    assert!(faint(&rest), "at rest a faint hue at each end: {rest:?}");
    assert!(rest.low.b > rest.low.r && rest.high.r > rest.high.b, "each end leans to its own side: {rest:?}");
    assert!(chroma(rest.between) <= 4, "the ground's grey between the ends: {rest:?}");
    // Twin: the hand's whole hues passed off as the rest's are caught.
    assert!(!faint(&hand), "whole hues at rest would be seen: {hand:?}");

    // The hand gone, the rail rests again; C's + key steps it and its hues show whole while the hand
    // stays on C's line, the key included; once the hand leaves the line they rest.
    to(p.get_to() + 40.0, 380.0);
    let plus = LogicalPosition::new(p.get_plus_x(), p.get_c_y());
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: plus });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: plus, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: plus, button: PointerEventButton::Left });
    for _ in 0..12 {
        rig.pump();
        std::thread::sleep(Duration::from_millis(16));
    }
    let stepped = cyan();
    println!("shot {}", rig.shot("slider-hues-stepped").display());
    assert_eq!(p.get_c(), 1, "the key stepped C once");
    assert!(chroma(stepped.low) > 100 && chroma(stepped.high) > 100, "a step shows C's red and cyan whole: {stepped:?}");
    rig.settle();
    let held = cyan();
    assert!(!faint(&held), "the hand still on C's line, its hues stay whole: {held:?}");
    to(p.get_to() + 40.0, 380.0);
    rig.settle();
    let after = cyan();
    assert!(faint(&after) && faint(&c_rest), "the hand off C's line, C rests faint again: {after:?} {c_rest:?}");
    assert!(after.low.r > after.low.b && after.high.b > after.high.r, "red at C's − end, cyan at its +: {after:?}");

    // The step keys are the interface's grey keycaps, legends in the neutral ink, at rest and under the hand.
    let (minus, plus_key) = (key_chroma(&rig, p.get_minus_x(), p.get_c_y()), key_chroma(&rig, p.get_plus_x(), p.get_c_y()));
    assert!(minus <= 6 && plus_key <= 6, "C's keys are grey: {minus} {plus_key}");
    // Twin: a key worn in its end's red, as they were, reads its colour by the same measure.
    let twin = key_chroma(&rig, p.get_twin_x(), p.get_twin_y());
    assert!(twin > 60, "a red key would be seen: {twin}");

    // D's rail is grey: whole at rest, dark at its − end and light at its +, as under the hand.
    to(p.get_keyed_from() + 0.25 * (p.get_keyed_to() - p.get_keyed_from()), p.get_d_y());
    let d_hand = dark();
    let spread = |e: &Ends| light(e.high) - light(e.low);
    println!("D rest {d_rest:?}\nD hand {d_hand:?}");
    assert!(spread(&d_rest) > 250 && (spread(&d_rest) - spread(&d_hand)).abs() < 40, "D stays whole: {} {}", spread(&d_rest), spread(&d_hand));
    // Twin: a rail laid on the ground at rest, as Temperature's, holds no such spread.
    assert!(spread(&rest).abs() < 100, "a faded rail's ends read alike: {}", spread(&rest));
}
