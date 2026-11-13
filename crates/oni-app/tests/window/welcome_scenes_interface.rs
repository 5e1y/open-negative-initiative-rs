//! The welcome's interface scene, held at the moments its direction reviews: the loop's two ends alike,
//! the look before's coloured keycaps at the opening and gone in the look now, the pouf between, and
//! the typed value close up in its focused field. `ONI_SCENE_FRAMES` also shoots every 100 ms.

use oni_app::{Help, MainWindow, SceneClock};
use slint::platform::WindowEvent;
use slint::platform::software_renderer::PremultipliedRgbaColor;
use slint::{ComponentHandle, PhysicalSize};
use std::path::{Path, PathBuf};

use crate::common;

type Rgb = (i32, i32, i32);
type Shot = (usize, Vec<Rgb>);
type Area = (usize, usize, usize, usize);

/// The window drawn at `held`, as rows of RGB; eight bits a channel, so a faint veil keeps its hue.
fn held(rig: &common::Rig, window: &MainWindow, at: f32) -> Shot {
    window.global::<SceneClock>().set_held(at);
    rig.pump();
    rig.window.request_redraw();
    let size = rig.window.size();
    let (w, h) = (size.width as usize, size.height as usize);
    let mut buffer = vec![PremultipliedRgbaColor::default(); w * h];
    rig.window.draw_if_needed(|r| {
        r.render(&mut buffer, w);
    });
    (w, buffer.iter().map(|p| (p.red as i32, p.green as i32, p.blue as i32)).collect())
}

fn near(p: Rgb, c: Rgb, by: i32) -> bool {
    (p.0 - c.0).abs() <= by && (p.1 - c.1).abs() <= by && (p.2 - c.2).abs() <= by
}

fn pixels((x0, y0, x1, y1): Area) -> impl Iterator<Item = (usize, usize)> {
    (y0..y1).flat_map(move |y| (x0..x1).map(move |x| (x, y)))
}

/// How many pixels of `area` lie near `c`.
fn count(shot: &Shot, area: Area, c: Rgb, by: i32) -> usize {
    pixels(area).filter(|&(x, y)| near(shot.1[y * shot.0 + x], c, by)).count()
}

/// How many pixels of `area` two shots differ by.
fn differ(a: &Shot, b: &Shot, area: Area) -> usize {
    pixels(area).filter(|&(x, y)| a.1[y * a.0 + x] != b.1[y * b.0 + x]).count()
}

/// The box of every pixel two shots differ by.
fn moved(a: &Shot, b: &Shot) -> Area {
    let w = a.0;
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for (i, (p, q)) in a.1.iter().zip(b.1.iter()).enumerate() {
        if p != q {
            let (x, y) = (i % w, i / w);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
        }
    }
    (x0, y0, x1, y1)
}

/// The stage alone, written as a PNG.
fn save(shot: &Shot, (x0, y0, x1, y1): Area, path: &Path) -> PathBuf {
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), (x1 - x0) as u32, (y1 - y0) as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let bytes: Vec<u8> = pixels((x0, y0, x1, y1)).flat_map(|(x, y)| {
        let p = shot.1[y * shot.0 + x];
        [p.0 as u8, p.1 as u8, p.2 as u8]
    }).collect();
    encoder.write_header().unwrap().write_image_data(&bytes).unwrap();
    path.to_path_buf()
}

pub(crate) fn the_interface_scene_shows_the_old_keycaps_then_the_new_look_and_a_typed_value() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots.clone());
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    let window = MainWindow::new().unwrap();
    oni_app::keycaps::serve(&window);
    let help = window.global::<Help>();
    help.set_release("1.6".into());
    help.set_welcome_page(1);
    help.set_welcome_shown(true);
    rig.window.set_size(PhysicalSize::new(1400, 1100));
    window.show().unwrap();
    rig.settle();

    let moments = [
        ("s0", 0.0), ("s1", 1.0), ("opening", 0.10), ("pouf-172", 0.172), ("pouf-215", 0.215), ("new", 0.33), ("typing", 0.585),
        ("pull-back", 0.80), ("rewind-015", 0.015), ("rewind-030", 0.030), ("knob", 0.70),
    ];
    let taken: Vec<Shot> = moments.iter().map(|&(_, at)| held(&rig, &window, at)).collect();
    let [s0, s1, opening, peak, _, new, typing, ..] = &taken[..] else { unreachable!() };

    // The stage is what the close view and the opening differ by: all of it, 560 by 373.
    let stage = moved(opening, typing);
    let (sx, sy) = (stage.2 - stage.0, stage.3 - stage.1);
    assert!(sx.abs_diff(560) <= 4 && sy.abs_diff(373) <= 4, "the stage is found whole: {stage:?}");
    for ((name, _), shot) in moments.iter().zip(taken.iter()) {
        println!("shot {}", save(shot, stage, &shots.join(format!("interface-{name}.png"))).display());
    }

    // The loop's two ends are one frame, the Reduce Motion one; twin: the opening is not that frame.
    assert_eq!(differ(s0, s1, stage), 0, "t = 0 and t = 1 show the same frame");
    assert!(differ(s0, opening, stage) > 1000, "the opening differs from the last frame");

    // The look before: the yellow reset and the red trash stand in the bar.
    let bar = (stage.0, stage.1, stage.2, stage.1 + 56);
    let (caution, destructive, accent) = ((224, 168, 20), (212, 51, 59), (10, 132, 255));
    assert!(count(opening, bar, caution, 14) > 40, "the old bar holds the yellow keycap");
    assert!(count(opening, bar, destructive, 14) > 40, "the old bar holds the red keycap");
    // Twin: the look now, the wrong moment for them, has neither.
    assert!(count(new, bar, caution, 14) < 4 && count(new, bar, destructive, 14) < 4, "the new bar holds no yellow or red keycap");
    // The pouf's peak is neither rest: the bar is between its two looks.
    assert!(differ(peak, opening, bar) > 200 && differ(peak, new, bar) > 200, "mid-pouf the bar is between its looks");

    // Close up, the typed value's field wears the accent's focus ring below where the bar was.
    let below = (stage.0 + 280, stage.1 + 60, stage.2, stage.3);
    assert!(count(typing, below, accent, 30) > 60, "the typed value's field wears the focus ring");
    // Twin: in the wide view at rest no ring shows there.
    assert!(count(new, below, accent, 30) < 4, "at rest the panel shows no focus ring");

    // For the director: every 100 ms of the loop, and the pouf every 40 ms, the stage alone.
    if std::env::var_os("ONI_SCENE_FRAMES").is_some() {
        let pouf = shots.join("interface-pouf");
        std::fs::create_dir_all(&pouf).unwrap();
        for step in 0..=30 {
            let at = 0.160 + 0.004 * step as f32;
            save(&held(&rig, &window, at), stage, &pouf.join(format!("t-{at:.3}.png")));
        }
        let frames = shots.join("interface-frames");
        std::fs::create_dir_all(&frames).unwrap();
        for ms in (0..=10_000).step_by(100) {
            let shot = held(&rig, &window, ms as f32 / 10_000.0);
            save(&shot, stage, &frames.join(format!("{ms:05}.png")));
        }
        println!("frames {}", frames.display());
    }
    window.global::<SceneClock>().set_held(-1.0);
    help.set_welcome_shown(false);
}
