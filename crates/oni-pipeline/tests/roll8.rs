//! Roll #8 through the whole pipeline against the Swift app's `--render` dumps at a 1000-pixel side:
//! fed the Swift decode, the render lands on the Swift one; fed LibRaw's, it moves by a measured amount.

use oni_graph::{Graph, Image, Source};
use oni_pipeline::{Inputs, Pipeline, Request};
use oni_settings::PipelineSettings;
use oni_settings::modes::{Face, ScannerMachine};
use oni_settings::scanner::ScannerClicks;
use std::path::Path;

/// The two frames checked by default; `ONI_FULL_ROLL=1` takes every frame the dump lists.
const FRAMES: [&str; 2] = ["DSC_6432.NEF", "DSC_6448.NEF"];

/// Fed the Swift decode, only the pipeline differs: Pro sits near 2e-4 of mean, the Scanner's cubes,
/// which Core Image samples coarser, near 1e-3.
const SWIFT_MEAN: f64 = 2e-3;
const SWIFT_P99: f64 = 1e-2;
/// Fed LibRaw's decode, the decoders' gap is added, 0.025 to 0.07 of mean on the roll: a bound on a
/// measured move, not a parity check, so it carries no twin.
const LIBRAW_MEAN: f64 = 0.1;

struct Dump {
    width: u32,
    height: u32,
    pixels: Vec<[f32; 4]>,
}

/// `RenderDump`'s format: width and height as `Int32`, then RGBA `Float`, top row first.
fn dump(path: &Path) -> Dump {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let int = |i: usize| i32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as u32;
    let (width, height) = (int(0), int(4));
    let floats: Vec<f32> = b[8..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    assert_eq!(floats.len(), (width * height * 4) as usize, "{}: truncated", path.display());
    Dump { width, height, pixels: floats.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect() }
}

/// Mean and 99th centile of the worst channel's gap, after the output clip both sides share.
fn gap(a: &[[f32; 4]], b: &[[f32; 4]]) -> (f64, f64) {
    assert_eq!(a.len(), b.len(), "renders of different sizes");
    let clip = |x: f32| x.clamp(0.0, 1.0);
    let mut e: Vec<f64> =
        a.iter().zip(b).map(|(x, y)| (0..3).map(|c| f64::from((clip(x[c]) - clip(y[c])).abs())).fold(0.0, f64::max)).collect();
    let mean = e.iter().sum::<f64>() / e.len() as f64;
    e.sort_by(|x, y| x.total_cmp(y));
    (mean, e[e.len() * 99 / 100])
}

fn luma(pixels: &[[f32; 4]]) -> f64 {
    pixels.iter().map(|p| f64::from(0.2627 * p[0] + 0.6780 * p[1] + 0.0593 * p[2])).sum::<f64>() / pixels.len() as f64
}

/// The frames to check, each with its index in the dump's list.
fn frames(dir: &Path) -> Vec<(usize, String)> {
    let list = std::fs::read_to_string(dir.join("list.txt")).expect("list.txt");
    let names: Vec<String> = list.lines().filter(|l| !l.is_empty()).map(|l| l.rsplit('/').next().unwrap().to_owned()).collect();
    if std::env::var_os("ONI_FULL_ROLL").is_some_and(|v| v == "1") {
        return names.into_iter().enumerate().collect();
    }
    println!("two frames; ONI_FULL_ROLL=1 takes the {} the dump lists", names.len());
    FRAMES.iter().map(|f| (names.iter().position(|n| n == f).unwrap_or_else(|| panic!("{f} is not in list.txt")), f.to_string())).collect()
}

fn render(g: &mut Graph, pipeline: &mut Pipeline, source: &Source, s: &PipelineSettings, request: &Request) -> (u32, u32, Vec<[f32; 4]>) {
    let image: Image = pipeline.image(g, source, s, request, &Inputs::default()).expect("the recipe builds");
    let e = image.extent();
    (e.w, e.h, g.render(&image, e, None).expect("the frame renders"))
}

#[test]
fn roll8_lands_on_the_swift_render() {
    let check = "roll8_lands_on_the_swift_render";
    let Some(dir) = oni_fixtures::reference_dir(check) else { return };
    let Some(roll) = oni_fixtures::raw_dir(check) else { return };
    if !oni_fixtures::heavy(check) {
        return;
    }
    let Ok(mut g) = Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")) else { return };
    let mut pipeline = Pipeline::new();
    // A frame whose NEF is not in the folder takes the roll's full width, read off the last one decoded.
    let mut roll_width = None;
    for (index, name) in frames(&dir) {
        let file = format!("{index:05}.f32");
        let (decode, swift) = (dump(&dir.join("decode").join(&file)), dump(&dir.join("render").join(&file)));
        let original = roll.join(&name);
        let settings = oni_settings::sidecar::read(&original, true, false).expect("the sidecar reads").settings.per_pixel_only();
        let full = original.exists().then(|| oni_decode::decode(&original, 1.0).expect("LibRaw decodes the frame"));
        let full_width = full.as_ref().map(|f| f.width as f64).or(roll_width);
        roll_width = full_width;

        // The Swift decode, at the Swift render's own reduced width over the frame's full one.
        let source = g.upload(decode.width, decode.height, &decode.pixels).unwrap();
        let request = Request { full_width, ..Request::default() };
        let (w, h, ours) = render(&mut g, &mut pipeline, &source, &settings, &request);
        assert_eq!((w, h), (swift.width, swift.height), "{name}: the frame's size departs from the Swift render's");
        let (mean, p99) = gap(&ours, &swift.pixels);
        println!("{name} {w}x{h}, on the Swift decode: mean {mean:.5}, p99 {p99:.5}; luma {:.4} for {:.4}", luma(&ours), luma(&swift.pixels));
        assert!(mean < SWIFT_MEAN && p99 < SWIFT_P99, "{name}: the pipeline departs from the Swift render");

        // Twins: two channels swapped, the face's own stage moved (Pro's print bypassed, the Scanner's
        // other machine, its clicks ignored), the capture's exposure ignored.
        let mut twins: Vec<(&str, Vec<[f32; 4]>)> = vec![("red and blue swapped", ours.iter().map(|p| [p[2], p[1], p[0], p[3]]).collect())];
        let mut moved = vec![];
        if settings.face == Face::Scanner {
            let other = match settings.scanner_machine {
                ScannerMachine::Frontier => ScannerMachine::Oni,
                ScannerMachine::Oni => ScannerMachine::Frontier,
            };
            moved.push(("the other machine", PipelineSettings { scanner_machine: other, ..settings.clone() }));
            if settings.scanner != ScannerClicks::default() {
                moved.push(("the clicks ignored", PipelineSettings { scanner: ScannerClicks::default(), ..settings.clone() }));
            }
        } else {
            let mut bypassed = settings.clone();
            bypassed.print.bypassed = true;
            moved.push(("print bypassed", bypassed));
        }
        for (what, s) in moved {
            twins.push((what, render(&mut g, &mut pipeline, &source, &s, &request).2));
        }
        if settings.capture_stops != 0.0 {
            let unlifted = PipelineSettings { capture_stops: 0.0, ..settings.clone() };
            twins.push(("capture stops ignored", render(&mut g, &mut pipeline, &source, &unlifted, &request).2));
        }
        for (what, twin) in twins {
            let (m, p) = gap(&twin, &swift.pixels);
            println!("  twin, {what}: mean {m:.5}, p99 {p:.5}");
            assert!(m >= SWIFT_MEAN || p >= SWIFT_P99, "{name}: twin {what} passes, so the check cannot see it");
        }

        // LibRaw's whole frame, reduced by the pipeline's own Lanczos to the Swift render's side.
        let Some(full) = full else {
            println!("  no {name} in the roll's folder: LibRaw's decode is not compared");
            continue;
        };
        let (fw, fh) = (full.width as u32, full.height as u32);
        let whole = g.upload(fw, fh, &full.pixels).unwrap();
        drop(full);
        let scale = f64::from(decode.width.max(decode.height)) / f64::from(fw.max(fh));
        let (lw, lh, libraw) = render(&mut g, &mut pipeline, &whole, &settings, &Request { scale, ..Request::default() });
        drop(whole);
        g.purge();
        // LibRaw's active area is not `CIRAWFilter`'s: the common top-left rows and columns are compared.
        let (cw, ch) = (lw.min(swift.width), lh.min(swift.height));
        let common = |pixels: &[[f32; 4]], width: u32| -> Vec<[f32; 4]> {
            (0..ch).flat_map(|y| (0..cw).map(move |x| (y * width + x) as usize)).map(|i| pixels[i]).collect()
        };
        println!("  LibRaw {fw}x{fh} reduced to {lw}x{lh}, compared on {cw}x{ch}");
        let (m, p) = gap(&common(&libraw, lw), &common(&swift.pixels, swift.width));
        println!("  on LibRaw's decode: mean {m:.5}, p99 {p:.5}; luma {:.4}", luma(&libraw));
        assert!(m < LIBRAW_MEAN, "{name}: LibRaw's render sits {m:.4} from the Swift one, past the measured move");
    }
}
