//! Roll #8, the reference roll, through the graph: the preview and measuring copies of every frame,
//! tiled against untiled and against the scalar reference, then a canvas region and a slider on one frame.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use common::{TOLERANCE, cut, error};
use oni_graph::cpu::{self, Plane};
use oni_graph::stages::{Axis, BoxBlur3, Gain};
use oni_graph::{Cache, Graph, Image, Rect};

/// `Negative.previewSide` and `Negative.measureSide`.
const PREVIEW_SIDE: f64 = 2000.0;
const MEASURE_SIDE: f64 = 1400.0;

/// Tiles small enough that every frame of the roll spans several.
const TILE: u32 = 2048;

fn frames(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("roll #8 unreadable at {}: {e}", dir.display()))
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("nef")))
        .collect();
    out.sort();
    assert!(!out.is_empty(), "no NEF in {}", dir.display());
    out
}

/// oni-decode builds LibRaw with `LIBRAW_NOTHREADS`: two decodes at once share its state and report
/// "data corrupted", so the tests, which run in parallel, decode one frame at a time.
static DECODING: Mutex<()> = Mutex::new(());

fn decode(path: &Path) -> oni_decode::Linear {
    let _one = DECODING.lock().unwrap_or_else(|e| e.into_inner());
    oni_decode::decode(path, 1.0).unwrap_or_else(|e| panic!("{} does not decode: {e:?}", path.display()))
}

/// The default adapter, or `None` after a skip line on a machine that has none.
fn graph(check: &str) -> Option<Graph> {
    Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")).ok()
}

fn reduced(image: Image) -> Image {
    let e = image.extent();
    let side = e.w.max(e.h) as f64;
    image.lanczos(PREVIEW_SIDE / side)
}

#[test]
fn every_frame_previews_as_its_reference() {
    let Some(dir) = oni_fixtures::raw_dir("every_frame_previews_as_its_reference") else { return };
    let Some(mut whole) = graph("every_frame_previews_as_its_reference") else { return };
    let mut tiled = Graph::with_max_side(TILE).expect("a second graph");
    println!("adapter: {}", whole.adapter);
    for path in frames(&dir) {
        let clock = Instant::now();
        let decoded = decode(&path);
        let decode_ms = clock.elapsed().as_secs_f64() * 1e3;
        let (w, h) = (decoded.width as u32, decoded.height as u32);
        let a = whole.upload(w, h, &decoded.pixels).expect("upload");
        let b = tiled.upload(w, h, &decoded.pixels).expect("upload");

        let clock = Instant::now();
        let preview = whole.materialise(&reduced(Image::new(&a))).expect("preview");
        let measuring = whole.materialise(&Image::new(&preview).lanczos(MEASURE_SIDE / PREVIEW_SIDE)).expect("measuring");
        let gpu_ms = clock.elapsed().as_secs_f64() * 1e3;
        let shown = whole.render(&Image::new(&preview), preview.extent(), None).expect("preview");
        let measured = whole.render(&Image::new(&measuring), measuring.extent(), None).expect("measuring");
        let side = preview.extent().w.max(preview.extent().h);
        assert_eq!(side, PREVIEW_SIDE as u32, "the preview's long side");
        assert_eq!(measuring.extent().w.max(measuring.extent().h), MEASURE_SIDE as u32);

        let held = tiled.materialise(&reduced(Image::new(&b))).expect("tiled preview");
        let shown_tiled = tiled.render(&Image::new(&held), held.extent(), None).expect("tiled preview");
        assert_eq!(shown, shown_tiled, "{}: the tiled preview differs", path.display());

        let plane = Plane::new(a.extent(), decoded.pixels);
        let e = preview.extent();
        let reference = cpu::lanczos(&cpu::lanczos(&plane, Axis::X, e.w), Axis::Y, e.h);
        let err = error(&shown, &reference.pixels);
        let m = measuring.extent();
        let measured_ref = cpu::lanczos(&cpu::lanczos(&reference, Axis::X, m.w), Axis::Y, m.h);
        let err_measured = error(&measured, &measured_ref.pixels);
        println!(
            "{}: {w}x{h}, {} tiles; decode {decode_ms:.0} ms, preview+measuring {gpu_ms:.0} ms; error {err:.1e}, measuring {err_measured:.1e}",
            path.file_name().unwrap().to_string_lossy(),
            b.tiles(),
        );
        assert!(b.tiles() > 1, "the tiled graph must cut the frame");
        assert!(err < TOLERANCE, "preview error {err}");
        assert!(err_measured < TOLERANCE, "measuring error {err_measured}");

        // Twin: the reference reduced from a frame softened by a 3 x 3 blur is told apart on real film.
        let soft = cpu::box_blur_3(&plane);
        let wrong = cpu::lanczos(&cpu::lanczos(&soft, Axis::X, e.w), Axis::Y, e.h);
        let err = error(&shown, &wrong.pixels);
        println!("twin, softened source: error {err:.1e}");
        assert!(err > TOLERANCE, "a softened source went unseen on real data: error {err}");
    }
}

#[test]
fn a_canvas_region_and_a_slider_on_one_frame() {
    let Some(dir) = oni_fixtures::raw_dir("a_canvas_region_and_a_slider_on_one_frame") else { return };
    let Some(mut g) = graph("a_canvas_region_and_a_slider_on_one_frame") else { return };
    let path = dir.join("DSC_6448.NEF");
    let decoded = decode(&path);
    let (w, h) = (decoded.width as u32, decoded.height as u32);
    let mut t = Graph::with_max_side(TILE).expect("a second graph");
    let a = g.upload(w, h, &decoded.pixels).expect("upload");
    let b = t.upload(w, h, &decoded.pixels).expect("upload");
    let chain = |s: &oni_graph::Source, late: f32| Image::new(s).then(Gain([1.6, 1.0, 0.7, 1.0])).then(BoxBlur3).then(Gain([late; 4]));

    // A 1:1 canvas view straddling the tile seams at 2048 px, against the full render cut.
    let view = Rect::new(2048 - 800, 2048 - 500, 1600, 1000);
    let full = g.render(&chain(&a, 1.0), a.extent(), None).expect("full");
    let clock = Instant::now();
    let region = g.render(&chain(&a, 1.0), view, None).expect("region");
    let region_ms = clock.elapsed().as_secs_f64() * 1e3;
    assert_eq!(region, cut(&full, a.extent(), view), "the region differs from the full render");
    assert_eq!(t.render(&chain(&b, 1.0), view, None).expect("tiled region"), region, "the seams show");

    // The slider: a late notch reruns one stage, an early one all three.
    let mut cache = Cache::default();
    g.render(&chain(&a, 1.0), view, Some(&mut cache)).expect("warm");
    let before = g.stats().dispatches;
    let clock = Instant::now();
    let late = g.render(&chain(&a, 1.1), view, Some(&mut cache)).expect("late notch");
    let late_ms = clock.elapsed().as_secs_f64() * 1e3;
    assert_eq!(g.stats().dispatches - before, 1, "a late notch reruns the last stage only");
    assert_eq!(late, g.render(&chain(&a, 1.1), view, None).expect("fresh"), "the cached notch differs from a fresh render");
    println!("{w}x{h}: 1600x1000 region {region_ms:.0} ms uncached, late notch {late_ms:.0} ms cached");

    // Twin: the picture before the notch, which a stale cache would show, fails the comparison.
    assert_ne!(region, late, "a stale picture went unseen");
}
