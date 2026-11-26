//! `virtual_negative <photo> <stock> <out.tif> [effects]`: a digital photo's frozen scan on `stock`, as the chain makes it
//! after the scanner and before any window or inversion, written as a linear 16-bit TIFF in the working space, its codes
//! the light a plain file reads back, framed by a margin of bare film for Prepare's black. `effects` keeps the stock's
//! grain and halation at their rests; without it both are off.

use oni_app::render::{self, Device, Engine, Reading};
use oni_graph::{Image, Rect};
use oni_pipeline::{Inputs, Request};
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};
use std::path::Path;

/// The bare film's margin on each side, a share of the frame's long side.
const MARGIN: f64 = 0.04;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (photo, stock, out) = (Path::new(&args[0]), VirtualStock::of(Box::leak(args[1].clone().into_boxed_str())), Path::new(&args[2]));
    assert!(stock.is_known(), "no stock file defines {}", args[1]);
    let effects = args.get(3).is_some_and(|a| a == "effects");
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true).picking(stock);
    if !effects {
        (s.grain.amount, s.halation.amount) = (0.0, 0.0);
    }
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    let negative = e.prepare(render::decode(photo, Reading::of(&s)).expect("a decode")).unwrap();
    let full = e.full(render::decode_full(photo, Reading::of(&s)).expect("a decode")).unwrap();
    let (graph, pipeline) = e.parts();
    let request = Request { full_width: Some(negative.full_width), ..Request::default() };
    let inputs = Inputs { patches: Default::default(), custom_flat_field: None, seed: negative.seed, plate: Some(&negative.preview) };
    let scan = pipeline.scan(graph, &full, &s, &request, &inputs).unwrap();
    let scan = graph.materialise_half(&scan).unwrap();
    // The bare film: the scan of no light, without the grain a margin's single pixel would not show.
    let bare = {
        let dark = graph.upload(4, 4, &[[0.0, 0.0, 0.0, 1.0]; 16]).unwrap();
        let plain = PipelineSettings { grain: oni_settings::blocks::Grain { amount: 0.0, ..s.grain }, ..s.clone() };
        let image = pipeline.scan(graph, &dark, &plain, &Request::default(), &Inputs::default()).unwrap();
        graph.render(&image, Rect::new(0, 0, 1, 1), None).unwrap()[0]
    };
    let r = scan.extent();
    let margin = (MARGIN * f64::from(r.w.max(r.h))).round() as u32;
    let (w, h) = (r.w + 2 * margin, r.h + 2 * margin);
    let code = |v: f32| (v.clamp(0.0, 1.0) * 65535.0).round() as u16;
    let mut samples = vec![0u16; (w * h * 4) as usize];
    for px in samples.chunks_exact_mut(4) {
        px.copy_from_slice(&[code(bare[0]), code(bare[1]), code(bare[2]), 65535]);
    }
    let (mut top, mut highest) = (0.0f32, 0u64);
    let rows = 1024;
    for y in (0..r.h).step_by(rows as usize) {
        let band = Rect::new(r.x, r.y + y as i32, r.w, rows.min(r.h - y));
        let pixels = graph.render(&Image::new(&scan), band, None).unwrap();
        for (k, p) in pixels.iter().enumerate() {
            let (x, yy) = (k as u32 % band.w + margin, k as u32 / band.w + y + margin);
            let at = ((yy * w + x) * 4) as usize;
            for c in 0..3 {
                top = top.max(p[c]);
                highest += u64::from(p[c] > 1.0);
                samples[at + c] = code(p[c]);
            }
            samples[at + 3] = 65535;
        }
    }
    let icc = oni_colour::space::linear_working_icc().expect("the working space's profile");
    let none = oni_export::ifd::Directory::default();
    let bytes = oni_export::tiff::encoded(w as usize, h as usize, &oni_colour::legs::Quantised::Sixteen(samples), true, &icc, &none, &none).expect("a TIFF");
    std::fs::write(out, bytes).unwrap();
    // Read back as the app reads a plain file: the margin's bare film is the light written, to the code's rounding.
    let back = render::decode_full(out, Reading::of(&PipelineSettings::rest(ConversionMode::Negative, false))).expect("the TIFF reads");
    let read = back.linear.pixels[(margin / 2 * w + margin / 2) as usize];
    assert!((0..3).all(|c| (read[c] - bare[c]).abs() < 2e-4), "the margin reads {read:?}, written {bare:?}");
    println!("{} on {}: {w} x {h}, bare film {:.4} {:.4} {:.4}, brightest {top:.4}, {highest} samples past 1 clipped", photo.display(), stock.name(), bare[0], bare[1], bare[2]);
}
