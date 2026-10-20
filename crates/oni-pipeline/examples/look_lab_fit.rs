//! `look_lab_fit [rounds]`: fits the Look Lab's Frontier set to the measured cube's constrained nodes
//! and writes both sets with their figures to `resources/scanner/lab.json`.

use oni_pipeline::look_lab::measure::{centile, constrained, delta_e, median};
use oni_pipeline::look_lab::{Chain, LabParams, SEED_FRONTIER, SEED_NEUTRAL, bake, fit, normalised};
use oni_pipeline::scanner::MACHINE;
use serde_json::json;

type Samples = Vec<([f64; 3], [f64; 3])>;

/// Median, p90 and p99 ΔE of a set against its targets.
fn figures(p: &LabParams, samples: &Samples) -> serde_json::Value {
    let chain = Chain::new(p);
    let e: Vec<f64> = samples.iter().map(|(x, t)| delta_e(chain.code(*x), *t)).collect();
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    json!({ "median": round(median(&e)), "p90": round(centile(&e, 0.9)), "p99": round(centile(&e, 0.99)), "samples": e.len() })
}

/// p99 ΔE of an `n`³ bake read by the cube kernel against the chain, on a fixed spread of inputs in reach.
fn baked(p: &LabParams, n: usize) -> f64 {
    let (cube, chain, (low, high)) = (bake(p, n), Chain::new(p), MACHINE.reached());
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let e: Vec<f64> = (0..100_000)
        .map(|_| {
            let x: [f64; 3] = std::array::from_fn(|c| {
                let lo = f64::from(low[c]).max(f64::from(MACHINE.low()));
                lo + next() * (f64::from(high[c]) - lo)
            });
            let t = x.map(|v| normalised(v) as f32);
            let read = oni_render::cpu::color_cube([t[0], t[1], t[2], 1.0], &cube);
            delta_e([read[0], read[1], read[2]].map(f64::from), chain.code(x))
        })
        .collect();
    (centile(&e, 0.99) * 1000.0).round() / 1000.0
}

/// Five decimals: the file stays readable and the character is taken against the rounded set.
fn rounded(p: &LabParams) -> LabParams {
    let v: Vec<f64> = fit::pack(p).iter().map(|x| (x * 1e5).round() / 1e5).collect();
    fit::unpack(&v, p.character)
}

fn main() {
    let rounds = std::env::args().nth(1).map_or(200, |r| r.parse().expect("a number of rounds"));
    let cube: Samples = constrained();
    println!("{} constrained nodes", cube.len());
    let seed = SEED_FRONTIER.with_character(0.0);
    println!("seed, paper only: {}", figures(&seed.paper_only(), &cube));
    println!("seed, full model: {}", figures(&seed, &cube));
    let frontier = rounded(&fit::fit(&seed, &cube, rounds, |round, cost| println!("  round {round}: mean ΔE² {:.4}", cost / cube.len() as f64)));
    let full = figures(&frontier, &cube);
    let paper = figures(&frontier.paper_only(), &cube);
    let bakes = json!({ "27": baked(&frontier, 27), "33": baked(&frontier, 33) });
    println!("fitted, full model: {full}\nfitted, colour stage at rest: {paper}\nbake p99 against the chain: {bakes}");
    let lab = json!({
        "neutral": SEED_NEUTRAL,
        "frontier": frontier.with_character(1.0),
        "figures": {
            "frontier": { "nodes": "inside reached, channel spread under 0.6", "model": full, "colourAtRest": paper, "bakeP99": bakes },
        },
        "source": "cargo run -p oni-pipeline --example look_lab_fit; the neutral set is the published seed",
    });
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/resources/scanner/lab.json");
    std::fs::write(path, serde_json::to_string_pretty(&lab).unwrap() + "\n").expect("lab.json");
    println!("wrote {path}");
}
