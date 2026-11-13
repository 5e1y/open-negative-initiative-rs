//! The welcome's third and fourth scenes held at the moments of their review: the first and last frame
//! alike, the opening, the middle and the end. The edits scene turns its cards while every picture's pixels
//! stay put; the fixes scene ticks and counts. Each assertion has a twin that looks where the change is.

use oni_app::{Help, MainWindow, SceneClock};
use slint::platform::software_renderer::Rgb565Pixel;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

use crate::common;

type Frame = Vec<Rgb565Pixel>;
type Box4 = (usize, usize, usize, usize);

/// The smallest box holding every pixel that differs between two frames of `width`, if any does.
fn changes(a: &Frame, b: &Frame, width: usize, within: Box4) -> Option<Box4> {
    let (x0, y0, x1, y1) = within;
    let mut found: Option<Box4> = None;
    for y in y0..y1 {
        for x in x0..x1 {
            if a[y * width + x].0 != b[y * width + x].0 {
                found = Some(match found {
                    None => (x, y, x + 1, y + 1),
                    Some((l, t, r, d)) => (l.min(x), t.min(y), r.max(x + 1), d.max(y + 1)),
                });
            }
        }
    }
    found
}

pub(crate) fn the_edits_scene_turns_its_cards_and_never_its_photos_and_the_fixes_tick() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let window = MainWindow::new().unwrap();
    oni_app::keycaps::serve(&window);
    let help = window.global::<Help>();
    help.set_release("1.6".into());
    help.set_welcome_line("V1.6.0".into());
    help.set_welcome_credit("".into());
    help.set_welcome_page(3);
    help.set_welcome_shown(true);
    let (w, h) = (1400u32, 1000u32);
    rig.window.set_size(PhysicalSize::new(w, h));
    window.show().unwrap();
    rig.settle();
    let clock = window.global::<SceneClock>();
    let width = w as usize;
    let whole = (0, 0, width, h as usize);
    let at = |held: f32, name: &str| -> Frame {
        clock.set_held(held);
        rig.pump();
        rig.pump();
        println!("shot {}", rig.shot(name).display());
        rig.drawn().1
    };
    // With ONI_SCENE_FRAMES set, a frame every 100 ms of the 7 s loop and the review's stills are shot too.
    let framed = std::env::var_os("ONI_SCENE_FRAMES").is_some();
    let frames = |scene: &str, stills: &[f32]| {
        if !framed {
            return;
        }
        for k in 0..=70 {
            at(k as f32 / 70.0, &format!("frame-{scene}-{k:03}"));
        }
        for t in stills {
            at(*t, &format!("still-{scene}-{:03}", (t * 1000.0).round() as u32));
        }
    };
    // The stage's corner in the window, found where the canvas colour begins.
    let (sx, sy) = ((width - 560) / 2, 144usize);
    let canvas = at(1.0, "welcome-edits-end");
    assert_ne!(canvas[(sy + 5) * width + sx + 5].0, canvas[(sy - 5) * width + sx + 5].0, "the stage starts where the check expects it");

    // The edits: eight cells in four columns, a picture 96 by 60 over a card 28 high, laid out as the scene does.
    let cell = |i: usize, thumb: bool| -> Box4 {
        let (c, r) = (i % 4, i / 4);
        let (x, y) = (sx + 40 + c * 128, sy + 55 + r * 124 + if thumb { 0 } else { 66 });
        (x + 2, y + 2, x + 94, y + if thumb { 58 } else { 26 })
    };
    let s0 = at(0.0, "welcome-edits-s0");
    let opening = at(0.12, "welcome-edits-opening");
    let middle = at(0.40, "welcome-edits-middle");
    let s1 = at(1.0, "welcome-edits-s1");
    assert!(changes(&s0, &s1, width, whole).is_none(), "the first and the last frame are the same");
    for i in 0..8 {
        for (name, frame) in [("s0", &s0), ("middle", &middle), ("end", &s1)] {
            assert!(changes(&opening, frame, width, cell(i, true)).is_none(), "picture {i} stays put to {name}");
        }
        assert!(changes(&opening, &s1, width, cell(i, false)).is_some(), "card {i} turns by the end");
    }
    // Twin: a box on a card, where the change is, must see it; a card is not a picture.
    assert!(changes(&opening, &s1, width, cell(0, false)).is_some());
    assert!(changes(&opening, &middle, width, cell(0, false)).is_some(), "the first card has turned by the middle");
    assert!(changes(&opening, &middle, width, cell(7, false)).is_none(), "the last card waits at the middle");
    assert!(changes(&middle, &s1, width, cell(7, false)).is_some(), "and turns after it");

    frames("edits", &[0.03, 0.196, 0.214, 0.243, 0.672]);

    // The fixes: the first and last frames alike, the rows ticking one after another, the count rising.
    help.set_welcome_page(4);
    rig.settle();
    let f0 = at(0.0, "welcome-fixes-s0");
    let opening = at(0.11, "welcome-fixes-opening");
    let half = at(0.40, "welcome-fixes-middle");
    let f1 = at(1.0, "welcome-fixes-s1");
    assert!(changes(&f0, &f1, width, whole).is_none(), "the fixes' first and last frame are the same");
    let ticked = changes(&opening, &f1, width, whole).expect("the fixes change between the opening and the end");
    let so_far = changes(&opening, &half, width, whole).expect("the fixes change by the middle");
    assert!(so_far != ticked, "the middle has ticked part of the list only");
    assert!(changes(&half, &f1, width, whole).is_some(), "the end ticks what the middle has not");
    // Twin: a frame held twice shows no change, so the differences above are the clock's.
    assert!(changes(&f1, &at(1.0, "welcome-fixes-s1-again"), width, whole).is_none());
    frames("fixes", &[0.016, 0.03, 0.032, 0.048, 0.175, 0.657]);
    clock.set_held(-1.0);
}
