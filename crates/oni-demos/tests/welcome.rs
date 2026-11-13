//! The welcome's staged films: a scene holds layout and wiring only; the interface film's typed figure
//! moves Temperature alone, to its place; its photograph never changes.

use std::path::{Path, PathBuf};

use i_slint_backend_testing::AccessibleRole;
use oni_app::fields::Field;
use oni_app::slider_track::{position_on, track_on};
use oni_demos::film::{self, FPS};
use oni_demos::photo::{Rgba, plain};
use oni_demos::stage::{SCALE, Stage};
use oni_demos::welcome::{self, interface};
use oni_settings::{ConversionMode, PipelineSettings};

#[test]
fn every_welcome_scene_holds_layout_and_wiring_only() {
    let mut files = 0;
    for entry in std::fs::read_dir(welcome::scenes()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "slint") {
            let faults = welcome::faults(&std::fs::read_to_string(&path).unwrap());
            assert!(faults.is_empty(), "{}: {faults:?}", path.display());
            files += 1;
        }
    }
    assert!(files > 0, "no scene read");
    // Twins: each thing a scene may not hold is seen, in code and not in a comment or a string.
    let clean = "// a Path, #ff0000, 4px\nRectangle { width: ShellStyle.panel-width; text: \"#fff 2px Path\"; }";
    assert!(welcome::faults(clean).is_empty(), "{:?}", welcome::faults(clean));
    for wrong in [
        "Rectangle { background: Colour.canvas; }",
        "Rectangle { x: 0; }\nText { color: #ff0000; }",
        "Path { commands: \"M 0 0\"; }",
        "Rectangle { width: 220px; }",
        "Rectangle { opacity: 0.5; animate opacity { duration: Motion.settle; } }",
        "Rectangle { property <brush> b; }",
    ] {
        assert!(!welcome::faults(wrong).is_empty(), "{wrong} passes");
    }
    assert_eq!(welcome::faults("A {}\n\nRectangle { width: 4px; }"), ["line 3: a size that is no token"]);
}

/// A gradient with a bright square, written as a plain image naming no profile.
fn picture(dir: &Path) -> PathBuf {
    let (w, h) = (2400u32, 1600u32);
    let pixels = (0..w * h)
        .flat_map(|i| {
            let (x, y) = (i % w, i / w);
            if (500..800).contains(&x) && (600..900).contains(&y) { [240, 230, 200, 255] } else { [(x / 10) as u8, (y / 7) as u8, 90, 255] }
        })
        .collect();
    let file = dir.join("picture.png");
    Rgba { width: w, height: h, pixels }.png(&file).unwrap();
    file
}

/// Points to the pixel rectangle they cover in a frame.
fn px(r: (f32, f32, f32, f32)) -> (u32, u32, u32, u32) {
    let p = |v: f32| (v * SCALE).round() as u32;
    (p(r.0), p(r.1), p(r.0 + r.2), p(r.1 + r.3))
}

/// The pixels where two frames differ.
fn changed(a: &Rgba, b: &Rgba) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for (i, (p, q)) in a.pixels.chunks_exact(4).zip(b.pixels.chunks_exact(4)).enumerate() {
        if p != q {
            out.push((i as u32 % a.width, i as u32 / a.width));
        }
    }
    out
}

#[test]
fn the_typed_figure_moves_temperature_alone_to_its_place_and_the_photograph_holds() {
    if oni_app::render::Device::open().is_err() {
        return println!("SKIPPED: no GPU, the film is not exercised");
    }
    let dir = std::env::temp_dir().join(format!("oni-demos-welcome-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = picture(&dir);
    let (typed, photo) = film::shoot(&mut interface::Interface::new(source.clone())).unwrap();
    let (untouched, _) = film::shoot(&mut interface::Interface::untouched(source.clone())).unwrap();

    // Where the scene lays its pieces, read off a stage of its own.
    let stage = Stage::new(&welcome::scenes().join("interface.slint"), "Interface", welcome::SIZE).unwrap();
    let row = interface::row(&stage).unwrap();
    let tabs = interface::placed(&stage, AccessibleRole::Tab, "Roll")[0];
    let title = interface::placed(&stage, AccessibleRole::Text, "Colour").into_iter().find(|r| r.1 > tabs.1 + tabs.3).unwrap();
    drop(stage);

    // The photograph's pixels are the export's, on every frame of both films.
    let strip = plain::cover(&source, (photo.width, photo.height), interface::CENTRE).unwrap();
    for frame in typed.iter().chain(&untouched) {
        for y in 0..photo.height {
            let at = (((photo.y + y) * frame.width + photo.x) * 4) as usize;
            let from = (y * photo.width * 4) as usize;
            assert!(frame.pixels[at..at + photo.width as usize * 4] == strip.pixels[from..from + photo.width as usize * 4], "photograph row {y}");
        }
    }

    // The hand rests on the number before its click and after Return: what changed between is the app's answer.
    let at = |s: f32| (s * FPS as f32).round() as usize;
    let (before, after) = (at(2.9), at(4.15));
    let moved = changed(&typed[before], &typed[after]);
    assert!(!moved.is_empty(), "the typed figure changed nothing");
    let (rx0, ry0, rx1, ry1) = px(row);
    let bar = px((0.0, 0.0, welcome::SIZE.0, tabs.1));
    // Colour's header row, whose Reset wakes beside the title.
    let header = px((0.0, title.1 - 8.0, welcome::SIZE.0, title.3 + 16.0));
    let inside = |(x, y): (u32, u32), r: (u32, u32, u32, u32)| x >= r.0 && x < r.2 && y >= r.1 && y < r.3;
    for p in &moved {
        assert!(inside(*p, (rx0, ry0, rx1, ry1)) || inside(*p, bar) || inside(*p, header), "pixel {p:?} changed outside Temperature's row and the Resets");
    }
    // The knob lands where the panel places −1.20 on Temperature's track, and not at its mirror.
    let s = PipelineSettings::rest(ConversionMode::Positive, false);
    let f = position_on(Field::Temperature, -1.2, &track_on(Field::Temperature, &s));
    assert!(f < 0.45, "−1.20 sits left of the middle: {f}");
    let rail = |p: &(u32, u32)| p.1 as f32 > (row.1 + row.3 * 0.6) * SCALE;
    let near = |x: f32| moved.iter().filter(|p| rail(p) && ((p.0 as f32 / SCALE) - x).abs() < 6.0).count();
    let (landed, mirror) = (row.0 + row.2 * f, row.0 + row.2 * (1.0 - f));
    assert!(near(landed) > 50, "the knob's place on the rail did not change: {}", near(landed));
    assert_eq!(near(mirror), 0, "twin: the rail changed where the knob did not go");

    // Twin: the hand resting on the number without a click changes nothing at all.
    assert!(changed(&untouched[before], &untouched[after]).is_empty(), "without a click the panel moved");
    let _ = std::fs::remove_dir_all(&dir);
}
