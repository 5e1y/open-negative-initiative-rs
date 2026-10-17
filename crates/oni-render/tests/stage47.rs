//! The WGSL kernel against the scalar reference, on settings that reach every branch of the kernel.

use oni_render::{cpu, gpu::Gpu, Stage47};

/// Largest error accepted, as |gpu - cpu| / (1 + |cpu|); set from the measured spread, see `measured`.
const TOLERANCE: f32 = 1e-4;

/// Transmittances a negative scan holds, from the densest highlight to just past the light panel.
fn pixels(n: usize) -> Vec<[f32; 4]> {
    let mut state = 0x2545_f491_u32;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32
    };
    (0..n)
        .map(|_| {
            let base = 10f32.powf(-2.6 * next());
            [base * (0.6 + 0.6 * next()), base * (0.5 + 0.5 * next()), base * (0.3 + 0.5 * next()), 1.0]
        })
        .collect()
}

fn cases() -> Vec<(&'static str, Stage47)> {
    let rest = Stage47::rest();
    let gains = Stage47 {
        gain: [2.1, 1.3, 0.9],
        red: [0.2, 1.6, 1.0],
        green: [0.25, 1.7, 0.95],
        blue: [0.3, 1.9, 1.05],
        pedestal: 0.001,
        log_guard: 1e-5,
        ..rest
    };
    let windows = Stage47 { low_points: [0.3, 0.22, 0.27], high_points: [0.7, 0.8, 0.74], ..gains };
    let decompressed = Stage47 {
        decompression: 0.7,
        film_share: 0.1,
        balance_low: [0.2, 0.25, 0.3],
        balance_span: [1.4, 1.45, 1.6],
        ..windows
    };
    let separated = Stage47 {
        sep_r: [1.2, -0.15, -0.05],
        sep_g: [-0.1, 1.25, -0.15],
        sep_b: [-0.05, -0.2, 1.25],
        ..decompressed
    };
    let printed = Stage47 {
        print_on: 1.0,
        print_stops: 1.5,
        print_contrast: 0.3,
        print_zones: [0.2, -0.4],
        own_stops: [0.1, -0.05, -0.08],
        hunt: 0.4,
        base_stops: 1.5,
        follow: 0.5,
        ..separated
    };
    let dense = Stage47 { density: 1.3, ..printed };
    let positive = Stage47 { invert: 0.0, window: 3.0, ..dense };
    let flattened = Stage47 { print_contrast: -0.5, print_zones: [0.6, -0.7], ..dense };
    vec![
        ("rest", rest),
        ("gains and levels", gains),
        ("window handles", windows),
        ("chroma decompression", decompressed),
        ("dye separation", separated),
        ("print, filters, Hunt, follow", printed),
        ("colour density", dense),
        ("positive mode", positive),
        ("negative contrast, double quarters", flattened),
    ]
}

fn error(gpu: &[[f32; 4]], cpu: &[[f32; 4]]) -> (f32, usize) {
    let mut worst = (0.0f32, 0);
    for (i, (g, c)) in gpu.iter().zip(cpu).enumerate() {
        for ch in 0..4 {
            let e = if g[ch].is_nan() != c[ch].is_nan() { f32::INFINITY } else { (g[ch] - c[ch]).abs() / (1.0 + c[ch].abs()) };
            if e > worst.0 {
                worst = (e, i);
            }
        }
    }
    worst
}

#[test]
fn measured() {
    let gpu = match Gpu::new() {
        Ok(g) => g,
        Err(e) => {
            println!("SKIPPED measured: no GPU adapter ({e:?})");
            return;
        }
    };
    println!("adapter: {}", gpu.adapter);
    let input = pixels(1 << 16);
    let mut failures = Vec::new();
    for (name, k) in cases() {
        let out = gpu.stage47(&input, &k);
        let reference: Vec<_> = input.iter().map(|p| cpu::stage47(*p, &k)).collect();
        let (err, at) = error(&out, &reference);
        // The twin: a 1 % green gain the reference does not see must break the tolerance.
        let shifted = Stage47 { gain: [k.gain[0], k.gain[1] * 1.01, k.gain[2]], ..k };
        let twin: Vec<_> = input.iter().map(|p| cpu::stage47(*p, &shifted)).collect();
        let (twin_err, _) = error(&out, &twin);
        println!("{name:<36} max {err:.3e} at {at:>5}   twin {twin_err:.3e}");
        if err > TOLERANCE {
            failures.push(format!("{name}: {err:e} > {TOLERANCE:e} at pixel {at}, in {:?}, gpu {:?}, cpu {:?}", input[at], out[at], reference[at]));
        }
        if twin_err <= TOLERANCE {
            failures.push(format!("{name}: twin {twin_err:e} within tolerance, the check cannot see a 1 % gain"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
