//! The export surface end to end on the headless window: roll #8 mirrored, the canvas proofed into
//! the export space, a frame written and read back by independent readers, a batch and its clash renamed.

use oni_app::app::export_card::{self, Ask};
use oni_app::app::{Options, Shell};
use oni_app::display::Screen;
use oni_app::export::label;
use oni_app::fields::Field;
use oni_app::{App, Bar, Exporting, Library, MainWindow, Store as StoreGlobal};
use oni_colour::Space;
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use crate::common;

const LONG: Duration = Duration::from_secs(600);

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-app-export-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Roll #8 as hard links beside copies of its sidecars, so no sidecar is written beside the originals.
fn mirror(roll: &Path, into: &Path) -> PathBuf {
    let dir = into.join("Roll #8");
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(roll).unwrap() {
        let from = entry.unwrap().path();
        let to = dir.join(from.file_name().unwrap());
        if from.extension().is_some_and(|e| e == "json") {
            std::fs::copy(&from, &to).unwrap();
        } else {
            std::fs::hard_link(&from, &to).unwrap();
        }
    }
    dir
}

/// Pumps until no render has landed for four seconds, longer than one render takes here.
fn quiet(rig: &common::Rig, renders: impl Fn() -> usize) {
    loop {
        let n = renders();
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(4) {
            rig.pump();
            std::thread::sleep(Duration::from_millis(20));
        }
        if renders() == n {
            return;
        }
    }
}

fn frame(w: &MainWindow) -> (usize, usize, Vec<u8>) {
    let buffer = w.global::<oni_app::Canvas>().get_frame().to_rgba8().expect("the canvas holds pixels");
    (buffer.width() as usize, buffer.height() as usize, buffer.as_bytes().to_vec())
}

/// Pixels whose channels differ by more than the dither's reach.
fn differing(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(4).zip(b.chunks_exact(4)).filter(|(p, q)| (0..3).any(|c| p[c].abs_diff(q[c]) > 3)).count()
}

/// Mean of each block of a `blocks` × `blocks` grid, as 0–1 values.
fn blocks(width: usize, height: usize, pixels: impl Fn(usize, usize) -> [f64; 3], n: usize) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    for by in 0..n {
        for bx in 0..n {
            let (x0, x1, y0, y1) = (bx * width / n, (bx + 1) * width / n, by * height / n, (by + 1) * height / n);
            let mut sum = [0.0; 3];
            let mut count = 0.0;
            for y in (y0..y1).step_by(2) {
                for x in (x0..x1).step_by(2) {
                    let p = pixels(x, y);
                    (0..3).for_each(|c| sum[c] += p[c]);
                    count += 1.0;
                }
            }
            out.push(sum.map(|s| s / count));
        }
    }
    out
}

fn worst(a: &[[f64; 3]], b: &[[f64; 3]]) -> f64 {
    a.iter().zip(b).flat_map(|(p, q)| (0..3).map(move |c| (p[c] - q[c]).abs())).fold(0.0, f64::max)
}

fn tags(file: &[u8]) -> (BTreeSet<u16>, exif::Exif) {
    let mut reader = exif::Reader::new();
    reader.continue_on_error(true);
    let read = match reader.read_raw(file.to_vec()) {
        Ok(f) => f,
        Err(exif::Error::PartialResult(p)) => p.into_inner().0,
        Err(e) => panic!("{e}"),
    };
    (read.fields().map(|f| f.tag.number()).collect(), read)
}

/// Fields of the scanning camera no line states: Make, serials, lens maker, aperture, exposure, focal length.
const FORBIDDEN: [u16; 7] = [271, 0xA431, 0xA433, 0xA435, 0x829D, 0x829A, 0x920A];

pub(crate) fn the_export_surface_writes_what_the_proofed_canvas_shows_and_only_onis_metadata() {
    let roll = oni_fixtures::folder(oni_fixtures::RAW_DIR);
    if !roll.join("DSC_6432.NEF").exists() {
        eprintln!("roll #8 is absent: the export surface is not exercised");
        return;
    }
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the export surface is not exercised"),
    };
    let shots = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots");
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let window = rig.window.clone();
    let root = scratch();
    let folder = mirror(&roll, &root);
    let support = root.join("support");
    let out = root.join("out");
    std::fs::create_dir_all(&out).unwrap();

    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    // A Display P3 screen, so a proof narrower than the screen is visible on it.
    let options = Options { screen: Screen::DisplayP3, canvas: false, support: support.clone(), caches: root.join("caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    window.set_size(PhysicalSize::new(1440, 900));
    shell.window.show().unwrap();
    shell.open_folder(&folder);
    let ctl = shell.ctl.clone();
    let w: &MainWindow = &shell.window;
    let e = w.global::<Exporting>();
    let asked: Rc<RefCell<Vec<Ask>>> = Rc::default();
    let answer: Rc<RefCell<Vec<PathBuf>>> = Rc::default();
    {
        let (asked, answer) = (asked.clone(), answer.clone());
        export_card::answer_with(&ctl, Rc::new(move |a: &Ask| {
            asked.borrow_mut().push(a.clone());
            answer.borrow().clone()
        }));
    }

    // The app opens on the gallery: the first photo is ringed and taken into the editor by ⌘G.
    rig.until("the library read", LONG, || ctl.state.borrow().read_done());
    w.global::<oni_app::Library>().invoke_tile_clicked(0, false, false);
    rig.pump();
    rig.chord(slint::platform::Key::Control, "g");
    assert!(ctl.state.borrow().open_path().is_some_and(|p| p.ends_with("DSC_6432.NEF")), "the first photo opens");
    rig.until("the first render", LONG, || ctl.state.borrow().renders() >= 1);
    let first = ctl.state.borrow().open_path().unwrap().to_string();
    assert!(first.ends_with("DSC_6432.NEF"), "{first}");
    // The four labels on one photo: the menu, the bar's key, the card's key, the context menu.
    assert_eq!(e.get_menu_title(), "Export…");
    assert!(e.get_menu_live());
    // The wheel along the quality slider moves it, as every slider; a scroll across it is the card's.
    let quality = e.get_quality_fraction();
    assert!(!e.invoke_quality_scroll(0.0, -40.0, 200.0, false), "a scroll across the track is the card's");
    assert!(e.invoke_quality_scroll(-40.0, 0.0, 200.0, false));
    assert!(e.get_quality_fraction() < quality, "{} from {quality}", e.get_quality_fraction());
    assert!(e.invoke_quality_scroll(40.0, 0.0, 200.0, false));
    assert!((e.get_quality_fraction() - quality).abs() < 1e-5, "twin: the way back lands where it began");
    assert_eq!(w.global::<Bar>().get_export_title(), "EXPORT");
    assert_eq!(label::context(1), "Export…");
    assert_eq!(e.get_chip_title(), "Display P3", "the export space is the canvas's, Display P3 at rest");
    assert_eq!(e.get_chip_hint(), Space::DisplayP3.hint());

    // Colour density raised so the frame holds colours sRGB cannot.
    let g = w.global::<StoreGlobal>();
    for _ in 0..40 {
        g.invoke_step(Field::ColorDensity.index(), 1);
    }
    let rendered = |n: usize| ctl.state.borrow().renders() >= n;
    let revision = ctl.state.borrow().store().revision();
    rig.until("the denser render", LONG, || ctl.state.borrow().rendered() >= revision);
    quiet(&rig, || ctl.state.borrow().renders());
    let (fw, fh, p3) = frame(w);

    // The canvas proofed into sRGB clips what a P3 screen would show.
    let n = ctl.state.borrow().renders();
    e.invoke_space_picked(0);
    rig.until("the sRGB proof", LONG, || rendered(n + 1));
    quiet(&rig, || ctl.state.borrow().renders());
    assert_eq!(e.get_chip_title(), "sRGB");
    let (_, _, srgb) = frame(w);
    let clipped = differing(&p3, &srgb);
    println!("sRGB proof moves {clipped} of {} pixels", fw * fh);
    assert!(clipped * 1000 > fw * fh, "the sRGB proof clips nothing visible: {clipped}");
    // Twin: ProPhoto is wider than the screen, so its proof shows the same picture as P3's.
    let n = ctl.state.borrow().renders();
    e.invoke_space_picked(3);
    rig.until("the ProPhoto proof", LONG, || rendered(n + 1));
    quiet(&rig, || ctl.state.borrow().renders());
    let (_, _, prophoto) = frame(w);
    let moved = differing(&p3, &prophoto);
    assert!(moved * 10_000 < fw * fh, "a space wider than the screen moved {moved} pixels");
    assert_eq!(e.get_format(), 0, "ProPhoto takes the TIFF along");
    let n = ctl.state.borrow().renders();
    e.invoke_space_picked(1);
    rig.until("back on P3", LONG, || rendered(n + 1));
    quiet(&rig, || ctl.state.borrow().renders());
    let (_, _, shown) = frame(w);

    // ⌘E opens the surface on the open photo; the bar and the library leave.
    rig.chord(slint::platform::Key::Control, "e");
    assert!(e.get_shown() && export_card::card(&ctl).shown);
    assert_eq!(e.get_count(), 1);
    assert_eq!(e.get_key_title(), "EXPORT");
    assert_eq!(e.get_menu_title(), "Export…");
    assert_eq!(e.get_subtitle(), "DSC_6432.NEF");
    assert_eq!(e.get_edited_in(), "", "edited and previewed in one space, the note says nothing");
    rig.until("the preview and the estimate", LONG, || {
        let l = export_card::lines(w);
        l.len() == 1 && l[0].state == oni_app::PreviewState::Shown && l[0].ready && e.get_weight() != "estimating…"
    });
    let line = &export_card::lines(w)[0];
    let line_camera = line.camera.to_string();
    assert_eq!(line.caption, e.get_dims(), "the cell and the summary name one size");
    assert!(e.get_dims().contains(" × ") && e.get_weight().starts_with("≈ "), "{} {}", e.get_dims(), e.get_weight());
    assert!(!line.camera.is_empty(), "the line opens on the sidecar's box or the scan's camera");
    // Another space for the previews says which one the frame was edited in.
    e.invoke_space_picked(0);
    assert_eq!(e.get_edited_in(), "This image was edited in Display P3. The previews show them in sRGB.");
    e.invoke_space_picked(1);
    assert_eq!(e.get_edited_in(), "");
    rig.until("the preview again", LONG, || export_card::lines(w)[0].state == oni_app::PreviewState::Shown);
    rig.settle();
    println!("shot {}", rig.shot("export-surface").display());

    // The line as typed, TIFF 16-bit, 2000 px on the long edge.
    e.invoke_edited(0, "artist".into(), "A. Tester".into());
    e.invoke_edited(0, "name".into(), "proofed".into());
    e.invoke_pick_film(0);
    let stocks = w.global::<oni_app::Stocks>();
    assert!(stocks.get_shown(), "the film field opens Select Roll Type");
    let gold = oni_settings::film::FILM_STOCKS.iter().position(|k| k.display_name == "Kodak Gold").expect("Kodak Gold listed");
    stocks.invoke_chosen(gold as i32);
    assert!(!stocks.get_shown());
    e.invoke_format_picked(0);
    assert_eq!(e.get_bits(), "16 bits per channel");
    let full_height = e.get_height_text();
    e.invoke_size_typed("2000".into(), full_height.clone());
    assert_eq!(e.get_width_text(), "2000", "a landscape's width is its long edge");
    assert!(e.get_resized());
    let destination = out.join("proofed.tiff");
    *answer.borrow_mut() = vec![destination.clone()];
    e.invoke_export();
    assert_eq!(asked.borrow().last(), Some(&Ask::Save { name: "proofed.tiff".into(), extension: "tiff".into() }));
    rig.until("the file written", LONG, || !export_card::card(&ctl).exporting && !e.get_shown());
    assert!(!w.global::<App>().get_gallery(), "the surface returns to the editor it opened from");

    // Read back by independent readers: size, compression, profile, the fields stated and no others.
    let file = std::fs::read(&destination).unwrap();
    let mut tiff = tiff::decoder::Decoder::new(Cursor::new(&file)).unwrap();
    let (tw, th) = tiff.dimensions().unwrap();
    assert_eq!(tw, 2000);
    assert_eq!(tiff.get_tag_u32(tiff::tags::Tag::Compression).unwrap(), 8, "ZIP");
    let icc = tiff.get_tag_u8_vec(tiff::tags::Tag::Unknown(34675)).unwrap();
    assert_eq!(Some(icc), Space::DisplayP3.icc_data(), "the file declares its space's profile");
    let (found, read) = tags(&file);
    assert!(FORBIDDEN.iter().all(|t| !found.contains(t)), "the scan leaks {found:?}");
    let text = |t| read.get_field(t, exif::In::PRIMARY).map(|f| f.display_value().to_string());
    // What the line held when written, field for field: stated, so the scan's own only when left there.
    assert_eq!(text(exif::Tag::Model), Some(format!("\"{}\"", line_camera)));
    assert_eq!(text(exif::Tag::Artist).as_deref(), Some("\"A. Tester\""));
    assert_eq!(text(exif::Tag::ImageDescription).as_deref(), Some("\"Kodak Gold\""));
    let date = oni_export::capture_date(&roll.join("DSC_6432.NEF")).unwrap();
    assert_eq!(text(exif::Tag::DateTimeOriginal), Some(date.replacen(':', "-", 2)));
    // Twin: the NEF itself carries the scan's fields this reader would have seen.
    let (scan, _) = tags(&std::fs::read(roll.join("DSC_6432.NEF")).unwrap());
    let carried = FORBIDDEN.iter().filter(|t| scan.contains(t)).count();
    assert!(scan.contains(&271) && carried >= 4, "the NEF carries {carried} of them");

    // The pixels are the canvas's: both Display P3, block means within a few levels.
    let pixels = match tiff.read_image().unwrap() {
        tiff::decoder::DecodingResult::U16(v) => v,
        other => panic!("not 16-bit: {:?}", std::mem::discriminant(&other)),
    };
    let channels = pixels.len() / (tw * th) as usize;
    let written = blocks(tw as usize, th as usize, |x, y| std::array::from_fn(|c| f64::from(pixels[(y * tw as usize + x) * channels + c]) / 65535.0), 12);
    let canvas = blocks(fw, fh, |x, y| std::array::from_fn(|c| f64::from(shown[(y * fw + x) * 4 + c]) / 255.0), 12);
    assert!((f64::from(tw) / f64::from(th) - fw as f64 / fh as f64).abs() < 0.01, "the file has the canvas's shape");
    let off = worst(&written, &canvas);
    println!("file against canvas: worst block {:.4}", off);
    assert!(off < 0.02, "the file is not what the canvas showed: {off}");
    // Twin: the same comparison against the canvas with red and blue swapped fails.
    let swapped: Vec<[f64; 3]> = canvas.iter().map(|p| [p[2], p[1], p[0]]).collect();
    assert!(worst(&written, &swapped) > 0.02);

    // The line kept beside the photo, the journal noted, the settings and the artist remembered.
    let exif = oni_settings::sidecar::read(Path::new(&first), true, false).unwrap().exif.expect("the line is kept in the sidecar");
    assert_eq!(exif.artist, "A. Tester");
    assert!(support.join("Journal/events.bin").exists());
    let prefs: serde_json::Value = serde_json::from_slice(&std::fs::read(support.join("preferences.json")).unwrap()).unwrap();
    assert_eq!(prefs["exportArtist"], "A. Tester");
    assert!(prefs["exportSettings"].as_str().unwrap().contains("tiff16"), "{}", prefs["exportSettings"]);

    // A batch from the gallery: the selection in its order, a folder, then the clash.
    ctl.act(oni_app::Act::ToggleMode);
    assert!(w.global::<App>().get_gallery());
    let lib = w.global::<Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(1, true, false);
    assert_eq!(e.get_menu_title(), "Export 2 Photos…");
    assert_eq!(w.global::<Bar>().get_export_title(), "EXPORT 2 PHOTOS");
    assert_eq!(label::context(2), "Export… (2 photos)");
    ctl.act(oni_app::Act::Export);
    assert!(e.get_shown());
    assert_eq!(e.get_key_title(), "EXPORT 2 PHOTOS");
    assert_eq!(e.get_menu_title(), "Export 2 Photos…");
    assert!(e.get_subtitle().starts_with("2 photos."));
    rig.until("the batch's lines", LONG, || export_card::lines(w).iter().all(|l| l.ready));
    let names: Vec<String> = export_card::lines(w).iter().map(|l| l.name.to_string()).collect();
    let batch = root.join("batch");
    std::fs::create_dir_all(&batch).unwrap();
    std::fs::write(batch.join(format!("{}.tiff", names[0])), b"kept").unwrap();
    *answer.borrow_mut() = vec![batch.clone()];
    e.invoke_export();
    assert_eq!(asked.borrow().last(), Some(&Ask::Folder { count: 2 }));
    assert!(e.get_confirm_shown());
    // A file ONI did not export there is offered Rename alone.
    assert_eq!(e.get_confirm_title(), "1 Name Is Already Taken in That Folder");
    assert!(!e.get_confirm_replaces());
    assert!(e.get_confirm_message().contains(&format!("{} 2.tiff", names[0])), "{}", e.get_confirm_message());
    // Twin: Cancel on the clash writes nothing.
    e.invoke_confirmed(false);
    assert!(!e.get_confirm_shown());
    rig.settle();
    assert_eq!(std::fs::read(batch.join(format!("{}.tiff", names[0]))).unwrap(), b"kept");
    assert!(!batch.join(format!("{}.tiff", names[1])).exists());
    e.invoke_export();
    e.invoke_confirmed(true);
    rig.until("the batch written", LONG, || !export_card::card(&ctl).exporting && !e.get_shown());
    assert_eq!(std::fs::read(batch.join(format!("{}.tiff", names[0]))).unwrap(), b"kept", "the file found there is kept");
    for name in [format!("{} 2.tiff", names[0]), format!("{}.tiff", names[1])] {
        let file = std::fs::read(batch.join(&name)).unwrap();
        assert!(file.starts_with(b"II") || file.starts_with(b"MM"), "{name} written as a TIFF");
    }
    assert!(w.global::<App>().get_gallery(), "a batch opened in the gallery returns there");
    let _ = std::fs::remove_dir_all(&root);
}
