//! The welcome's De-dust scene held at the direction's moments: the loop's ends identical, the
//! opening's dust, the reading's sweep, the heal. A speck dark on the sky heals into it.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Help, MainWindow, SceneClock};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

use crate::common;

/// The checked speck's centre in the scene: `WelcomeDedust.picture-x/-y` plus speck 7's `x`/`y`.
const SPECK: (f32, f32) = (18.0 + 136.0, 75.0 + 84.0);
/// Beside it on the same row, where the sky has the speck's own colour and no other speck lies.
const BESIDE: f32 = 9.0;

fn rgb(rig: &common::Rig, x: f32, y: f32) -> [i32; 3] {
    let (w, buffer) = rig.drawn();
    let v = buffer[y.round() as usize * w + x.round() as usize].0;
    [((v >> 11) & 0x1f) as i32 * 255 / 31, ((v >> 5) & 0x3f) as i32 * 255 / 63, (v & 0x1f) as i32 * 255 / 31]
}

fn luma(c: [i32; 3]) -> i32 {
    (c[0] * 3 + c[1] * 6 + c[2]) / 10
}

/// The speck's spot the colour of the sky beside it, within the 565 buffer's steps.
fn healed(rig: &common::Rig, at: (f32, f32)) -> bool {
    let spot = rgb(rig, at.0, at.1);
    let around = [rgb(rig, at.0 - BESIDE, at.1), rgb(rig, at.0 + BESIDE, at.1)];
    around.iter().all(|c| (0..3).all(|k| (spot[k] - c[k]).abs() <= 12))
}

/// The scene's pixels, row by row.
fn stage(rig: &common::Rig, scene: &common::Placed) -> Vec<u16> {
    let (w, buffer) = rig.drawn();
    let (x0, y0) = (scene.x.round() as usize, scene.y.round() as usize);
    (y0..y0 + scene.height as usize).flat_map(|y| (x0..x0 + scene.width as usize).map(move |x| (y, x))).map(|(y, x)| buffer[y * w + x].0).collect()
}

pub(crate) fn the_dedust_scene_rings_then_heals_every_speck() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let window = MainWindow::new().unwrap();
    oni_app::keycaps::serve(&window);
    let help = window.global::<Help>();
    help.set_release("1.6".into());
    help.set_welcome_page(2);
    help.set_welcome_shown(true);
    rig.window.set_size(PhysicalSize::new(1000, 900));
    window.show().unwrap();
    let clock = window.global::<SceneClock>();
    clock.set_held(0.0);
    rig.settle();

    let scene = common::the(&window, Role::Image, "Automatic De-dust");
    assert_eq!((scene.width, scene.height.round()), (560.0, 373.0), "the scene fills the wide sheet's stage");
    let at = (scene.x + SPECK.0, scene.y + SPECK.1);

    // The loop's two ends are one frame, the Reduce Motion one.
    let mut ends = Vec::new();
    for (name, moment) in [("s0", 0.0), ("s1", 1.0)] {
        clock.set_held(moment);
        rig.pump();
        println!("shot {}", rig.shot(&format!("welcome-dedust-{name}")).display());
        ends.push(stage(&rig, &scene));
    }
    assert!(ends[0] == ends[1], "the frames at 0 and 1 are identical");

    let mut seen = Vec::new();
    for (name, moment) in [("opening", 0.07), ("pointer", 0.155), ("sweep", 0.27), ("ringed", 0.40), ("remove", 0.525), ("heal", 0.68), ("end", 0.90)] {
        clock.set_held(moment);
        rig.pump();
        println!("shot {}", rig.shot(&format!("welcome-dedust-{name}")).display());
        seen.push((name, luma(rgb(&rig, at.0, at.1)) - luma(rgb(&rig, at.0 - BESIDE, at.1)), healed(&rig, at)));
    }
    println!("{seen:?}");
    let (opening, end) = (seen[0], seen[6]);
    assert!(opening.1 < -40, "the speck is darker than the sky beside it at the start: {seen:?}");
    assert!(end.2, "the speck's spot matches the sky around it once healed: {seen:?}");
    // Twin: on the opening the same test sees the speck, so it tells a healed spot from a dusty one.
    assert!(!opening.2, "at 0.07 the speck is not healed: {seen:?}");
    // A different frame is told apart: the opening is not the end.
    clock.set_held(0.07);
    rig.pump();
    assert!(stage(&rig, &scene) != ends[1], "twin: the opening differs from the end");
    // For a review of the motion: one frame every 100 ms of the 9 s loop, named by its ms.
    if std::env::var_os("ONI_SCENE_FRAMES").is_some() {
        for ms in (0..9000).step_by(100) {
            clock.set_held(ms as f32 / 9000.0);
            rig.pump();
            rig.shot(&format!("welcome-dedust-frame-{ms:04}"));
        }
    }
    clock.set_held(-1.0);
    help.set_welcome_shown(false);
}
