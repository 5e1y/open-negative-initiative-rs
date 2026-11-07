//! Prepare's frames by the hand: the keys between frames named by their frame, never Back's and
//! NEXT's words; the loupe's disc showing the film it reads; a scrub dissolving each frame in.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::{Bar, Library, Preparing};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition, Model};
use std::time::{Duration, Instant};

use crate::common;

/// A dissolve's length, `prepare::card::FADE`: two make a flash, under three a second.
const FADE: Duration = Duration::from_millis(200);

/// The luma of the drawn window's pixel holding a point, out of 255 per channel summed.
fn luma(drawn: &(usize, Vec<slint::platform::software_renderer::Rgb565Pixel>), (x, y): (f32, f32)) -> u32 {
    let v = drawn.1[y.floor() as usize * drawn.0 + x.floor() as usize].0;
    u32::from((v >> 11) & 0x1f) * 255 / 31 + u32::from((v >> 5) & 0x3f) * 255 / 63 + u32::from(v & 0x1f) * 255 / 31
}

pub(crate) fn the_frame_keys_name_their_frame_the_loupe_shows_its_film_and_a_scrub_dissolves() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Prepare's frames are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-prepare-frames-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Scans");
    let names = ["A.tif", "B.tif", "C.tif", "D.tif", "E.tif"];
    for (n, tone) in names.iter().zip([0.9, 0.3, 0.8, 0.2, 0.7]) {
        common::graded_tiff(&folder.join(n), 600, 400, tone);
    }
    let rig = common::rig(std::env::temp_dir());
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), require_roll: true, ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let p = w.global::<Preparing>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(60), || ctl.state.borrow().read_done());
    let first = oni_app::app::tiles(w).into_iter().min_by_key(|t| t.index).unwrap().index;
    w.global::<Library>().invoke_tile_clicked(first, false, false);
    rig.pump();
    w.global::<Bar>().invoke_act(oni_app::Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_secs(30), || p.get_shown());

    // The base step: the arrows either side of the frame's name are the frame's keys, the footer's
    // Back and NEXT the step's; no key says Previous or Next alone.
    p.invoke_step_picked(2);
    rig.until("the scan read", Duration::from_secs(60), || p.get_scan_ready() && p.get_base_name() == "A.tif");
    rig.settle();
    let (before, after) = (common::the(w, Role::Button, "Previous Frame"), common::the(w, Role::Button, "Next Frame"));
    assert!(common::placed(w, Role::Button, "Previous").is_empty() && common::placed(w, Role::Button, "Next").is_empty());
    // Twin: the search finds the step's own keys, named by the step.
    let (back, next) = (common::the(w, Role::Button, "Back"), common::the(w, Role::Button, "NEXT"));
    assert!(before.level_with(&after) && back.level_with(&next) && after.y < back.y, "the frame's keys over the scan, the step's in the footer");
    common::click(&rig, w, after.middle());
    rig.until("the next frame's scan", Duration::from_secs(60), || p.get_scan_ready() && p.get_base_name() == "B.tif");
    common::click(&rig, w, before.middle());
    rig.until("the first frame's scan again", Duration::from_secs(60), || p.get_scan_ready() && p.get_base_name() == "A.tif");
    assert_eq!(p.get_step(), 2, "twin: the frame's keys never walk the steps");

    // The loupe over the bare film: its disc shows the film it reads, bounded by a white ring on a
    // dark halo drawn as a border, never a shadow filling the disc.
    let at = (p.get_scan_x() + p.get_scan_width() * 0.4, p.get_scan_y() + p.get_scan_height() * 0.4);
    let hover = || {
        let _ = rig.drawn();
        let position = LogicalPosition::new(p.get_base_origin_x() + at.0, p.get_base_origin_y() + at.1);
        rig.window.dispatch_event(WindowEvent::PointerMoved { position });
        rig.pump();
    };
    rig.until("the loupe's full resolution", Duration::from_secs(60), || {
        hover();
        p.get_loupe_shown() && p.get_loupe_cut().row_data(2).unwrap_or(0) > 0
    });
    let rings = p.get_loupe_rings();
    let corner = (p.get_base_origin_x() + p.get_loupe_x() - 80.0, p.get_base_origin_y() + p.get_loupe_y() - 80.0);
    let (cx, cy, r) = (corner.0 + rings.row_data(0).unwrap(), corner.1 + rings.row_data(1).unwrap(), rings.row_data(2).unwrap() / 2.0);
    assert!(r >= 4.0, "a disc wide enough to look into: {r}");
    let drawn = rig.drawn();
    // Along the disc's radius: the white ring 1 pt either side of its edge, the halo the point past it.
    let across = |from: f32, to: f32| (0..=4).map(|i| luma(&drawn, (cx + from + (to - from) * i as f32 / 4.0, cy))).collect::<Vec<u32>>();
    let (centre, film, ring, halo) = (luma(&drawn, (cx, cy)), luma(&drawn, (cx + r + 6.0, cy)), across(r - 1.0, r + 1.0), across(r + 1.0, r + 2.5));
    println!("loupe: centre {centre}, film {film}, ring {ring:?}, halo {halo:?}");
    assert!(centre.abs_diff(film) < 40, "the disc shows the film it reads: {centre} against {film}");
    assert!(ring.iter().max().is_some_and(|l| *l > film + 60), "the white ring: {ring:?} against {film}");
    assert!(halo.iter().min().is_some_and(|l| l * 100 < film * 88), "its dark halo: {halo:?} against {film}");
    // Twin: inside the disc, past the halo's inner edge, the film again, never darkened.
    assert!(luma(&drawn, (cx + r - 3.5, cy)).abs_diff(film) < 40);

    // The window step: a scrub asks a frame at every move, the card dissolving each in once the last
    // has ended, so it changes at most once a dissolve and ends on the frame asked last.
    p.invoke_step_picked(4);
    rig.until("the card on the first frame", Duration::from_secs(60), || ctl.state.borrow().roll_card().is_some_and(|c| c.ends_with("A.tif")));
    std::thread::sleep(FADE);
    let (mut flips, mut asked) = (Vec::new(), Vec::new());
    let (mut second, mut name) = (p.get_card_second(), p.get_frame_name());
    let start = Instant::now();
    for i in 0..60 {
        p.invoke_slider_step(4, if (i / 4) % 2 == 0 { 1 } else { -1 });
        rig.pump();
        if p.get_card_second() != second {
            second = !second;
            flips.push(Instant::now());
        }
        if p.get_frame_name() != name {
            name = p.get_frame_name();
            asked.push(Instant::now());
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    println!("scrub of {:.0} ms: {} frames asked, {} dissolves", start.elapsed().as_secs_f64() * 1000.0, asked.len(), flips.len());
    let least = |t: &[Instant]| t.windows(2).map(|w| w[1] - w[0]).min().unwrap_or(Duration::MAX);
    assert!(flips.len() >= 3 && least(&flips) + Duration::from_millis(10) >= FADE, "one dissolve at a time: {:?}", least(&flips));
    // Twin: the hand asked frames far faster, which a card following it would flash.
    assert!(asked.len() > 2 * flips.len() && least(&asked) < FADE / 2, "{:?}", least(&asked));
    let last = name.to_string();
    rig.until("the frame asked last on the card", Duration::from_secs(10), || ctl.state.borrow().roll_card().is_some_and(|c| c.ends_with(&last)));
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
