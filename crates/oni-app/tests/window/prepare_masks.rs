//! Prepare the Roll's masks on the headless window: Load… and Use This Frame as Flat-Field as Swift
//! sets them, the frames' previews and the measure laid through the roll's custom mask.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::flat_fields;
use oni_app::{Advice, Bar, Library, MaskDialog, Preparing};
use oni_core::{Archive, EditorStore};
use oni_settings::sidecar;
use slint::platform::software_renderer::Rgb565Pixel;
use slint::{ComponentHandle, Model};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use crate::common;

fn stops(path: &Path) -> [f32; 3] {
    sidecar::read(path, false, true).unwrap().settings.roll_reference.expect("measured").stops
}

/// The mean distance of two pictures' bytes, over their colour channels.
fn apart(a: &slint::Image, b: &slint::Image) -> f64 {
    let (a, b) = (a.to_rgba8().expect("pixels"), b.to_rgba8().expect("pixels"));
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let sum: f64 = a.as_slice().iter().zip(b.as_slice()).map(|(p, q)| [p.r, p.g, p.b].iter().zip([q.r, q.g, q.b]).map(|(x, y)| f64::from(x.abs_diff(y))).sum::<f64>()).sum();
    sum / (3 * a.as_slice().len()) as f64
}

/// A 565 pixel's channels scaled to 8 bits, and their relative luminance.
fn rgb(p: Rgb565Pixel) -> [f64; 3] {
    let v = p.0;
    [f64::from((v >> 11) & 0x1f) * 255.0 / 31.0, f64::from((v >> 5) & 0x3f) * 255.0 / 63.0, f64::from(v & 0x1f) * 255.0 / 31.0]
}

fn luminance(c: [f64; 3]) -> f64 {
    let linear = |v: f64| if v <= 10.31 { v / 255.0 / 12.92 } else { ((v / 255.0 + 0.055) / 1.055).powf(2.4) };
    0.2126 * linear(c[0]) + 0.7152 * linear(c[1]) + 0.0722 * linear(c[2])
}

fn contrast(a: f64, b: f64) -> f64 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The step bar's done check reads 3:1 on its dimmed disc; a step's caution keeps its orange in the
/// triangle, its words in the panel's ink.
fn the_flat_field_step_reads_its_check_and_its_caution(rig: &common::Rig, w: &oni_app::MainWindow, p: &Preparing) {
    // Drawn at 2 pixels a point, as on the Mac: at 1 the check's strokes are thinner than a pixel.
    let k = 2.0;
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: k });
    rig.window.set_size(slint::PhysicalSize::new(2560, 1600));
    rig.settle();
    let (width, px) = rig.drawn();
    let at = |x: f32, y: f32| rgb(px[(y * k) as usize * width + (x * k) as usize]);
    let film = common::placed(w, Role::Text, "Film Type").into_iter().find(|t| t.y < w.global::<Bar>().get_height()).expect("the film step in the bar");
    // The disc, an icon's 16 points, stands 4 points before its word.
    let (cx, cy) = (film.x - 12.0, film.y + film.height / 2.0);
    let ring = |from: f32, to: f32| {
        let mut found = Vec::new();
        for dy in -16..=16 {
            for dx in -16..=16 {
                let (x, y) = (dx as f32 / k, dy as f32 / k);
                if (from..=to).contains(&(x * x + y * y).sqrt()) {
                    found.push(at(cx + x, cy + y));
                }
            }
        }
        found
    };
    let mut fill: Vec<f64> = ring(6.5, 7.0).into_iter().map(luminance).collect();
    fill.sort_by(f64::total_cmp);
    let fill = fill[fill.len() / 2];
    let check = ring(0.0, 6.0).into_iter().map(luminance).fold(0.0, f64::max);
    assert!(contrast(check, fill) >= 3.0, "the done check reads 3:1 on its disc: {:.2}", contrast(check, fill));
    // Twin: the accent's blue at the idle tab's 62 % over the bar reads under it.
    let hollow = at(cx - 11.0, cy);
    let accent: Vec<f64> = [10.0, 132.0, 255.0].iter().zip(hollow).map(|(a, h)| 0.62 * a + 0.38 * h).collect();
    let blue = luminance([accent[0], accent[1], accent[2]]);
    assert!(contrast(blue, fill) < 3.0, "twin: an accent check falls short: {:.2}", contrast(blue, fill));

    // Set as the roll's frames would say it, and drawn before anything publishes it away.
    let why = "Two frames were shot apart.";
    p.set_differ(why.into());
    let (width, px) = rig.drawn();
    let (title, words) = (common::the(w, Role::Text, "Settings differ across the roll"), common::the(w, Role::Text, why));
    let orange = |x0: f32, y0: f32, x1: f32, y1: f32| {
        let mut n = 0;
        for y in (y0 * k) as usize..(y1 * k) as usize {
            for x in (x0 * k) as usize..(x1 * k) as usize {
                let [r, g, b] = rgb(px[y * width + x]);
                n += usize::from(r > 180.0 && r - b > 120.0 && g > 80.0);
            }
        }
        n
    };
    assert_eq!(orange(words.x, words.y, words.x + words.width, words.y + words.height), 0, "the words in the panel's ink");
    assert!(orange(words.x, title.y, title.x, title.y + title.height) > 0, "twin: the triangle before the title is the warning's orange");
    p.set_differ("".into());
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    rig.settle();
}

pub(crate) fn the_roll_loads_and_takes_its_mask_and_shows_and_measures_through_it_as_swift_does() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Prepare the Roll's masks are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-prepare-masks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for roll in ["R1", "R2", "R3"] {
        for (name, tone) in [("A.tif", 0.9), ("B.tif", 0.7), ("C.tif", 0.5)] {
            common::graded_tiff(&scans.join(roll).join(name), 120, 80, tone);
        }
    }
    // The panel falls off from its centre, as a light panel's does.
    let panel_file = root.join("Rig/panel.tif");
    common::pixels_tiff(&panel_file, 120, 80, |x, y| {
        let r = ((x as f32 - 60.0).powi(2) + (y as f32 - 40.0).powi(2)) / (60.0f32.powi(2) + 40.0f32.powi(2));
        [0.9 - 0.4 * r; 3]
    });
    let noise = root.join("Rig/noise.tif");
    std::fs::write(&noise, b"no picture in here").unwrap();
    let path = |roll: &str, n: &str| PathBuf::from(oni_library::paths::canonical(&scans.join(roll).join(n)));
    let chosen: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let asked = chosen.clone();
    let options = Options { support: support.clone(), caches, picker: Box::new(|| None), mask_picker: Box::new(move || asked.borrow().clone()), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, p, advice, dialog) = (w.global::<Library>(), w.global::<Preparing>(), w.global::<Advice>(), w.global::<MaskDialog>());
    shell.open_folder(&scans);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().photos() == 9);
    rig.settle();
    let index = |roll: &str, name: &str| {
        let wanted = path(roll, name).to_string_lossy().into_owned();
        ctl.state.borrow().order().iter().position(|p| *p == wanted).unwrap_or_else(|| panic!("{roll}/{name}")) as i32
    };
    // A camera's roll, so the flat-field step stands; its frames' previews landed.
    let prepare = |roll: &str| {
        lib.invoke_tile_clicked(index(roll, "A.tif"), false, false);
        rig.pump();
        w.global::<Bar>().invoke_act(oni_app::Act::PrepareRoll);
        rig.until("the roll's surface", Duration::from_secs(10), || p.get_shown() && p.get_cells().row_count() == 3);
        rig.until("every frame read", Duration::from_secs(10), || p.get_capture() == 0);
        p.invoke_capture_picked(1);
        p.invoke_step_picked(1);
        rig.pump();
        rig.until("the previews", Duration::from_secs(60), || ctl.state.borrow().roll_previewed() == 3);
    };
    let tile = |i: usize| p.get_cells().row_data(i).unwrap().picture;
    let line = |title: &str| (0..p.get_mask_lines().row_count()).find(|&i| p.get_mask_lines().row_data(i).unwrap().text == title).unwrap_or_else(|| panic!("{title}")) as i32;
    let measure = || {
        p.invoke_step_picked(4);
        rig.pump();
        p.invoke_advance();
        rig.until("the measure landed", Duration::from_secs(120), || !p.get_shown() || advice.get_shown());
        assert!(!advice.get_shown(), "{}: {}", advice.get_title(), advice.get_message());
    };

    prepare("R1");
    assert_eq!(p.get_step(), 1);
    assert!(!p.get_can_take_flat() && !p.get_loading_flat(), "no frame picked yet");
    let bare = tile(0);
    the_flat_field_step_reads_its_check_and_its_caution(&rig, w, &p);

    // Load…: a cancelled panel does nothing; a file no reader takes says so; the panel's picture
    // becomes the roll's mask, its Amount as the hand left it, and no offer to move it out.
    p.invoke_load_flat();
    rig.pump();
    assert!(!p.get_loading_flat() && flat_fields::entries(&support).is_empty());
    assert!(!p.invoke_press(0, false));
    p.invoke_drag(0, -60.0, 200.0, false);
    p.invoke_release(0);
    rig.pump();
    let dialled = p.get_readouts().row_data(0).unwrap();
    assert_ne!(dialled.as_str(), "100%");
    *chosen.borrow_mut() = Some(noise.clone());
    p.invoke_load_flat();
    rig.until("the noise refused", Duration::from_secs(60), || !p.get_loading_flat());
    assert_eq!((advice.get_title().as_str(), advice.get_message().as_str()), ("No Mask Loaded", "This file could not be decoded as a picture."));
    advice.invoke_closed();
    *chosen.borrow_mut() = Some(panel_file.clone());
    p.invoke_load_flat();
    assert!(p.get_loading_flat(), "Loading… while the decode runs");
    rig.until("the panel loaded", Duration::from_secs(60), || !p.get_loading_flat());
    assert_eq!(flat_fields::entries(&support).iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["panel"]);
    assert!(p.get_flat_on() && p.get_flat_masked() && p.get_mask_title() == "panel");
    assert_eq!(p.get_readouts().row_data(0).unwrap(), dialled, "a mask loaded keeps the Amount");
    assert!(!dialog.get_shown() && !advice.get_shown());

    // The frames' previews follow the mask; switched off, they come back as they were.
    rig.until("the previews through the mask", Duration::from_secs(30), || apart(&tile(0), &bare) > 4.0);
    println!("through the mask: {:.2}", apart(&tile(0), &bare));
    rig.settle();
    println!("shot {}", rig.shot("prepare-masks-loaded").display());
    p.invoke_mask_chosen(line("None selected"));
    rig.until("twin: the previews without it", Duration::from_secs(30), || apart(&tile(0), &bare) < 1.0);
    println!("without: {:.2}", apart(&tile(0), &bare));

    // Use This Frame as Flat-Field: B picked, decoded as the mask at the whole Amount, then left out.
    assert_eq!(p.get_readouts().row_data(0).unwrap(), dialled);
    p.invoke_cell_clicked(1);
    rig.pump();
    assert!(p.get_can_take_flat());
    p.invoke_take_flat();
    assert!(p.get_taking_flat(), "Setting… while the decode runs");
    rig.until("B taken", Duration::from_secs(60), || !p.get_taking_flat());
    assert!(!advice.get_shown(), "{}", advice.get_message());
    assert!(p.get_mask_title() == "B" && p.get_readouts().row_data(0).unwrap() == "100%", "taken at the whole Amount");
    assert!(!p.get_cells().row_data(1).unwrap().ticked, "the mask's frame leaves the measure");
    assert!(p.get_cells().row_data(0).unwrap().ticked && p.get_cells().row_data(2).unwrap().ticked, "twin: the others stay");
    assert!(!dialog.get_shown() && path("R1", "B.tif").exists(), "the roll offers no move out");
    // A frame no reader takes says so under Swift's title, and the mask stays.
    std::fs::write(path("R1", "C.tif"), b"no longer a picture").unwrap();
    p.invoke_cell_clicked(2);
    p.invoke_take_flat();
    rig.until("C refused", Duration::from_secs(60), || !p.get_taking_flat());
    assert_eq!((advice.get_title().as_str(), advice.get_message().as_str()), ("No Mask Set", "This file could not be decoded as a picture."));
    advice.invoke_closed();
    assert!(p.get_mask_title() == "B" && p.get_cells().row_data(2).unwrap().ticked);
    common::graded_tiff(&path("R1", "C.tif"), 120, 80, 0.5);

    // The measure reads through the roll's mask: R1 on the panel, R2 bare, R3 on the panel again,
    // A and C voting on each, B left out or flagged.
    p.invoke_mask_chosen(line("panel"));
    measure();
    let flag_b = || {
        p.invoke_step_picked(3);
        rig.pump();
        p.invoke_sheet_clicked(1);
        assert!(p.get_cells().row_data(1).unwrap().flagged);
    };
    prepare("R2");
    flag_b();
    measure();
    prepare("R3");
    p.invoke_mask_chosen(line("panel"));
    flag_b();
    measure();
    let (masked, bare, again) = (stops(&path("R1", "A.tif")), stops(&path("R2", "A.tif")), stops(&path("R3", "A.tif")));
    println!("stops on the mask {masked:?}, bare {bare:?}, again {again:?}");
    assert!((0..3).any(|c| (masked[c] - bare[c]).abs() > 1e-3), "the mask reaches the measure");
    assert!((0..3).all(|c| (masked[c] - again[c]).abs() < 1e-4), "twin: the same mask measures the same");

    // A mask still loading when its roll is left lands on no other roll: a pipe holds the read
    // until R2 shows, then gives the panel's bytes.
    #[cfg(unix)]
    {
        let pipe = root.join("Rig/slow.tif");
        assert!(std::process::Command::new("mkfifo").arg(&pipe).status().unwrap().success());
        let panel = std::fs::read(&panel_file).unwrap();
        let give = |pipe: &Path| {
            let (pipe, panel) = (pipe.to_path_buf(), panel.clone());
            std::thread::spawn(move || std::fs::write(pipe, panel).unwrap());
        };
        prepare("R1");
        *chosen.borrow_mut() = Some(pipe.clone());
        p.invoke_load_flat();
        assert!(p.get_loading_flat());
        w.global::<Bar>().invoke_act(oni_app::Act::LeaveRoll);
        rig.pump();
        prepare("R2");
        p.invoke_mask_chosen(line("None selected"));
        rig.pump();
        assert!(!p.get_flat_masked());
        give(&pipe);
        rig.until("the late mask landed", Duration::from_secs(30), || !p.get_loading_flat());
        rig.settle();
        assert!(!p.get_flat_masked(), "R1's mask never lands on R2");
        // Twin: the same load with R2 still shown names the panel as its mask.
        p.invoke_load_flat();
        give(&pipe);
        rig.until("the mask landed", Duration::from_secs(30), || !p.get_loading_flat());
        assert!(p.get_flat_masked() && p.get_mask_title() == "panel");
    }
    let _ = std::fs::remove_dir_all(&root);
}
