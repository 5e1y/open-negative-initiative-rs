use super::measure::*;
use super::*;

/// Median ΔE of a chain against the cube on the constrained nodes.
fn against_cube(chain: &Chain) -> f64 {
    let e: Vec<f64> = constrained().iter().map(|(x, cube)| delta_e(chain.code(*x), *cube)).collect();
    median(&e)
}

#[test]
fn oklab_goes_there_and_back() {
    let rgb = [0.2, 0.5, 0.05];
    let back = linear_rgb(oklab(rgb));
    assert!((0..3).all(|c| (back[c] - rgb[c]).abs() < 1e-6), "{back:?}");
    let white = oklab([1.0; 3]);
    assert!((white[0] - 1.0).abs() < 1e-6 && white[1].abs() < 1e-6 && white[2].abs() < 1e-6, "{white:?}");
    // Twin: a channel swapped lands elsewhere.
    let swapped = linear_rgb(oklab([rgb[2], rgb[1], rgb[0]]));
    assert!((swapped[0] - rgb[0]).abs() > 0.1);
}

#[test]
fn the_constrained_nodes_are_the_measured_core_of_the_cube() {
    let (reached, constrained) = (reached().len(), constrained().len());
    println!("{reached} reached nodes, {constrained} constrained");
    assert_eq!(reached, 7581, "38.5 % of the 27³ lattice");
    assert_eq!(constrained, 1440);
}

/// The paper stage at Frontier values stays near the cube; made common to the channels it does not.
#[test]
fn the_paper_model_lands_near_the_cube() {
    let frontier = against_cube(&Chain::new(&SEED_FRONTIER.paper_only()));
    println!("paper model at Frontier values against the cube: median ΔE {frontier:.3}");
    assert!(frontier <= 2.0, "{frontier}");
    // Twin: the identity matrix, the gains and offsets made common to all three channels.
    let common = LabParams { matrix: [0.0; 6], paper_gain: [1.0; 2], paper_offset: [0.0; 2], ..SEED_FRONTIER.paper_only() };
    let twin = against_cube(&Chain::new(&common));
    println!("  twin, identity and common paper: {twin:.3}");
    assert!(twin > 2.5, "{twin}");
}

/// Both pivots the probe tried: the machine's neutral green passes, density zero does not.
#[test]
fn the_paper_turns_around_the_neutral_green() {
    let paper = SEED_FRONTIER.paper_only();
    let neutral = against_cube(&Chain::with_pivot(&paper, pivot()));
    let zero = against_cube(&Chain::with_pivot(&paper, 0.0));
    println!("pivot {:.4}: {neutral:.3}; pivot 0: {zero:.3}", pivot());
    assert!((pivot() - 0.7244).abs() < 1e-6);
    assert!(neutral <= 2.0 && zero > 2.0, "{neutral} {zero}");
}

/// Random densities inside the reach, from a fixed seed.
fn inside_reach(count: usize) -> Vec<[f64; 3]> {
    let (low, high) = MACHINE.reached();
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    (0..count)
        .map(|_| {
            std::array::from_fn(|c| {
                let lo = f64::from(low[c]).max(f64::from(MACHINE.low()));
                lo + next() * (f64::from(high[c]) - lo)
            })
        })
        .collect()
}

fn bake_error(n: usize, inputs: &[[f64; 3]]) -> f64 {
    let paper = SEED_FRONTIER.paper_only();
    let (cube, chain) = (bake(&paper, n), Chain::new(&paper));
    let e: Vec<f64> = inputs
        .iter()
        .map(|x| {
            let t = x.map(|v| normalised(v) as f32);
            let read = oni_render::cpu::color_cube([t[0], t[1], t[2], 1.0], &cube);
            delta_e([read[0], read[1], read[2]].map(f64::from), chain.code(*x))
        })
        .collect();
    centile(&e, 0.99)
}

/// The 27³ bake the kernel reads stays within sight of the paper chain it samples.
#[test]
fn the_bake_holds_the_chain() {
    let inputs = inside_reach(100_000);
    let p99 = bake_error(27, &inputs);
    println!("27³ bake against the chain: p99 ΔE {p99:.3}");
    assert!(p99 < 0.5, "{p99}");
    // Twin: a 9³ lattice is too coarse for the paper's S.
    let coarse = bake_error(9, &inputs);
    println!("  twin, 9³: {coarse:.3}");
    assert!(coarse >= 0.5, "{coarse}");
}

/// The neutral chain slid along the grey axis until its grey prints at the cube's grey lightness.
fn reference() -> (f64, Chain) {
    let neutral = MACHINE.neutral().map(f64::from);
    let cube_grey = {
        let t = neutral.map(|v| normalised(v) as f32);
        let c = oni_render::cpu::color_cube([t[0], t[1], t[2], 1.0], &MACHINE.render);
        lab_of([c[0], c[1], c[2]].map(f64::from))[0]
    };
    let chain = Chain::new(&SEED_NEUTRAL.paper_only());
    let (mut lo, mut hi) = (-1.0, 0.0);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if lab_of(chain.code(neutral.map(|v| v + mid)))[0] > cube_grey { hi = mid } else { lo = mid }
    }
    (lo, chain)
}

/// A lattice `k` times finer than the cube's inside the reach, each point with the cube's reading.
fn finer(k: usize) -> Vec<([f64; 3], [f64; 3])> {
    let n = (SIDE - 1) * k + 1;
    let (low, high) = MACHINE.reached();
    (0..n * n * n)
        .map(|i| [i % n, (i / n) % n, i / (n * n)].map(|j| node(j, n)))
        .filter(|x| (0..3).all(|c| x[c] >= f64::from(low[c]) && x[c] <= f64::from(high[c])))
        .map(|x| {
            let t = x.map(|v| normalised(v) as f32);
            let c = oni_render::cpu::color_cube([t[0], t[1], t[2], 1.0], &MACHINE.render);
            (x, [c[0], c[1], c[2]].map(f64::from))
        })
        .collect()
}

/// The probe of the plan's diagnosis: skin and blue nodes picked on the neutral reference.
#[test]
fn the_cube_turns_skin_and_blue_and_green_borrowing_red_carries_the_skin() {
    let (shift, neutral) = reference();
    println!("reference: neutral slid {shift:.4} along grey");
    let nodes = finer(4);
    let at = |chain: &Chain, x: [f64; 3]| lch(lab_of(chain.code(x)));
    let pick = |hue: (f64, f64), chroma: (f64, f64), light: (f64, f64)| -> Vec<([f64; 3], [f64; 3])> {
        nodes
            .iter()
            .filter(|(x, _)| {
                let r = at(&neutral, x.map(|v| v + shift));
                (hue.0..=hue.1).contains(&r[2]) && (chroma.0..=chroma.1).contains(&r[1]) && (light.0..=light.1).contains(&r[0])
            })
            .cloned()
            .collect()
    };
    let skin = pick((50.0, 66.0), (0.04, 0.09), (0.55, 0.80));
    let blue = pick((238.0, 258.0), (0.06, 0.14), (0.40, 0.70));
    println!("{} skin, {} blue nodes", skin.len(), blue.len());
    let turned = |set: &[([f64; 3], [f64; 3])], f: &dyn Fn([f64; 3], [f64; 3]) -> f64| median(&set.iter().map(|(x, c)| f(*x, *c)).collect::<Vec<_>>());
    let cube_turn = |set: &[([f64; 3], [f64; 3])]| turned(set, &|x, c| turn(lch(lab_of(c))[2], at(&neutral, x.map(|v| v + shift))[2]));
    let (skin_cube, blue_cube) = (cube_turn(&skin), cube_turn(&blue));
    println!("cube against the reference: skin {skin_cube:+.1}°, blue {blue_cube:+.1}°");
    let frontier = Chain::new(&SEED_FRONTIER.paper_only());
    let mut moved = SEED_FRONTIER.paper_only();
    moved.matrix[2] = SEED_NEUTRAL.matrix[2];
    let moved = Chain::new(&moved);
    let skin_back = turned(&skin, &|x, _| turn(at(&moved, x)[2], at(&frontier, x)[2]));
    println!("green borrows red at neutral: skin {skin_back:+.1}°");
    assert!(skin_cube > 5.0 && blue_cube > 5.0, "{skin_cube} {blue_cube}");
    assert!(skin_back < -5.0, "{skin_back}");
    // Twin: the same measure on the reference against itself finds no turn, so the check is not met by default.
    let still = turned(&skin, &|x, _| turn(at(&neutral, x.map(|v| v + shift))[2], at(&neutral, x.map(|v| v + shift))[2]));
    assert!(still.abs() < 1e-9, "{still}");
}
