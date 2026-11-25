//! The grain's RMS granularity as a data sheet measures it: the spread of the film's density through a 48 µm aperture,
//! times 1000, for each Grain size at a photo's resting Grain, on the neutral film; in JSON, through which
//! scripts/datasheet-profiles.py reads a sheet's figure back. Run: cargo run -p oni-pipeline --example grain_granularity.

use oni_graph::Rect;
use oni_pipeline::film::{self, neutral};
use oni_pipeline::{conversion, grain};
use oni_render::cpu;
use oni_settings::blocks::Grain;
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};

/// The micrometres of film a pixel of the measured patch stands for.
const PIXEL: f64 = 2.0;
/// The micro-densitometer's aperture, micrometres across.
const APERTURE: f64 = 48.0;
/// The apertures read along each side of the patch.
const READS: usize = 64;

fn main() {
    let rest = PipelineSettings::rest(ConversionMode::Digital, true);
    let stock = VirtualStock::NEUTRAL;
    let (reading, range) = (conversion::stage47(&rest), f64::from(neutral::kernel(stock).expect("the neutral film").range));
    // A flat scene at half the window: the grain's spread is uniform in the film's density, so any tone reads alike.
    let light = (0..60).fold((0.0f32, film::saturation()), |(lo, hi), _| {
        let mid = 0.5 * (lo + hi);
        if cpu::windowed(film::shot(stock, [mid, mid, mid, 1.0], 0.0), &reading)[1] < 0.5 { (mid, hi) } else { (lo, mid) }
    });
    let flat = film::shot(stock, [light.0, light.0, light.0, 1.0], 0.0);
    let long = (Grain::FRAME_LONG / PIXEL) as u32;
    let across = (APERTURE / PIXEL).round() as usize;
    let inside: Vec<(usize, usize)> = (0..across * across)
        .map(|i| (i % across, i / across))
        .filter(|(x, y)| {
            let (dx, dy) = (*x as f64 + 0.5 - across as f64 / 2.0, *y as f64 + 0.5 - across as f64 / 2.0);
            dx * dx + dy * dy <= (across * across) as f64 / 4.0
        })
        .collect();
    let (mut sizes, mut rms) = (Vec::new(), Vec::new());
    let mut size = Grain::SIZE_RANGE.0;
    while size <= Grain::SIZE_RANGE.1 {
        let mut s = rest.clone();
        s.grain.size = size;
        let k = grain::grain(&s, Rect::new(0, 0, long, long * 2 / 3), 1.0).expect("a resting photo is grained").dissolving(0.0);
        let means: Vec<f64> = (0..READS * READS)
            .map(|i| {
                let (ox, oy) = ((i % READS) * across, (i / READS) * across);
                let density = |(x, y): &(usize, usize)| {
                    let at = [(ox + x) as f32 + 0.5, (oy + y) as f32 + 0.5];
                    f64::from(cpu::windowed(cpu::grain(flat, flat, at, &k), &reading)[1]) * range
                };
                inside.iter().map(density).sum::<f64>() / inside.len() as f64
            })
            .collect();
        let mean = means.iter().sum::<f64>() / means.len() as f64;
        let spread = (means.iter().map(|m| (m - mean) * (m - mean)).sum::<f64>() / means.len() as f64).sqrt();
        sizes.push(f64::from(size));
        rms.push(1000.0 * spread);
        size += 2.5;
    }
    println!("{}", serde_json::json!({ "amount": rest.grain.amount, "window": range, "sizes": sizes, "rms": rms }));
}
