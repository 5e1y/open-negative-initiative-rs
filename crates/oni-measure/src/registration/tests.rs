use super::*;

const SIDE: usize = 900;

/// Discs of many sizes and levels over a grey ground, edges in every direction, none on the grid
/// the cells use: `RGBRegistration.selfCheck`'s textured frame.
fn scene() -> Plane {
    let (w, h) = (SIDE, (SIDE as f64 * 0.7) as usize);
    let mut values = vec![0.3f32; w * h];
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    for _ in 0..420 {
        let level = (0.05 + 0.9 * next()) as f32;
        let (x, y) = (next() * w as f64, next() * h as f64);
        let (r0, r1) = (3.0 + 14.0 * next(), 3.6 + 14.0 * next());
        let (inner, outer) = (r0.min(r1), r0.max(r1).max(r0.min(r1) + 0.6));
        let (lo_x, hi_x) = (((x - outer).floor().max(0.0)) as usize, ((x + outer).ceil() as usize).min(w));
        let (lo_y, hi_y) = (((y - outer).floor().max(0.0)) as usize, ((y + outer).ceil() as usize).min(h));
        for row in lo_y..hi_y {
            for col in lo_x..hi_x {
                let r = ((col as f64 + 0.5 - x).powi(2) + (row as f64 + 0.5 - y).powi(2)).sqrt();
                let alpha = ((outer - r) / (outer - inner)).clamp(0.0, 1.0) as f32;
                let v = &mut values[row * w + col];
                *v = level * alpha + *v * (1.0 - alpha);
            }
        }
    }
    Plane { width: w, height: h, values }
}

fn worst(cells: &[Cell]) -> f64 {
    if cells.is_empty() {
        return f64::INFINITY;
    }
    cells.iter().map(|c| c.dx.abs().max(c.dy.abs())).fold(0.0, f64::max)
}

/// A pass magnified and shifted by known amounts reads them back,
/// laid back by the fit it lands on the green, and an untouched pass reads as identity.
#[test]
fn a_pass_s_magnification_turn_and_shift_read_back_off_its_edges() {
    let green = scene();
    let truth = Field { k: 6e-4, theta: 4e-4, tx: 0.7, ty: -0.45 };
    // The pass is read at x + d(x): built as the green laid by d's inverse.
    let frame = transform(&fit(&truth, SIDE), green.width, green.height);
    let pass = transformed(&green, &inverted(&frame));
    let found = cells(&green, &pass, 7, 96, 4);
    let field = fitted(&found, green.width, green.height).expect("a field read");
    let half = SIDE as f64 / 2.0;
    let error = ((field.k - truth.k).abs() * half).max((field.theta - truth.theta).abs() * half).max((field.tx - truth.tx).abs()).max((field.ty - truth.ty).abs());
    assert!(error < 0.08, "read back within {error:.3} px at the frame's edge over {} cells", found.len());
    let given = shift(&found, truth.k, green.width, green.height).expect("a shift read");
    let off = (given.tx - truth.tx).abs().max((given.ty - truth.ty).abs()).max((given.theta - truth.theta).abs() * half);
    assert!(off < 0.06, "the magnification given, turn and shift within {off:.3} px");
    // Laid back by the fit read, the pass lands on the green: the residual is the reading's, and
    // the two bilinear resamplings', which this frame's 49 textured cells show at their worst.
    let back = transformed(&pass, &transform(&fit(&field, SIDE), green.width, green.height));
    let backs = cells(&green, &back, 7, 96, 4);
    let mut offs: Vec<f64> = backs.iter().map(|c| c.dx.abs().max(c.dy.abs())).collect();
    offs.sort_by(f64::total_cmp);
    let (left, middle) = (worst(&backs), offs[offs.len() / 2]);
    assert!(backs.len() == 49 && left < 0.2 && middle < 0.06, "laid back, the pass lies within {left:.3} px of the green in every cell, {middle:.3} at the median");
    // Twin: laid the wrong way, by the fit's inverse, the pass lies twice as far off.
    let wrong = transformed(&pass, &inverted(&transform(&fit(&field, SIDE), green.width, green.height)));
    assert!(worst(&cells(&green, &wrong, 7, 96, 4)) > 1.0, "the fit's direction is what lays the pass");
    // Twin: an untouched pass reads as no magnification, no turn and no shift.
    let identity = fitted(&cells(&green, &green, 7, 96, 4), green.width, green.height).expect("identity read");
    assert!(identity.k.abs().max(identity.theta.abs()) * half < 0.02 && identity.tx.abs() < 0.02 && identity.ty.abs() < 0.02, "{identity:?}");
}

/// The roll's two readings through injected planes: the red pass carried, the blue untouched.
#[test]
fn a_roll_reads_its_magnification_once_and_each_frame_s_shift_on_its_own() {
    let green = scene();
    let truth = Field { k: 6e-4, theta: 0.0, tx: 0.7, ty: -0.45 };
    let red = transformed(&green, &inverted(&transform(&fit(&truth, SIDE), green.width, green.height)));
    let planes = |_: &str, _: &RgbPasses, _: Option<f64>| Some([red.clone(), green.clone(), green.clone()]);
    let passes = RgbPasses { red: "r".into(), green: "g".into(), blue: "b".into(), registration: None };
    let frames = vec![("/roll/b".to_string(), passes.clone())];
    let (kr, kb) = magnification(&frames, &planes).expect("a magnification read");
    assert!((kr - truth.k).abs() * (SIDE as f64 / 2.0) < 0.08 && kb.abs() * (SIDE as f64 / 2.0) < 0.02, "red {kr:e}, blue {kb:e}");
    let r = registration("/roll/b", &passes, (kr, kb), &planes).expect("a registration read");
    let expected = fit(&truth, SIDE);
    assert!((r.red.dx - expected.dx).abs() * (SIDE as f64) < 0.08 && (r.red.dy - expected.dy).abs() * (SIDE as f64) < 0.08, "{:?} against {expected:?}", r.red);
    assert!(r.blue.dx.abs() * (SIDE as f64) < 0.02 && (r.blue.scale - 1.0).abs() < 1e-4, "{:?}", r.blue);
    // Twin: a frame whose passes cannot be read gives no registration, and no roll no magnification.
    let none = |_: &str, _: &RgbPasses, _: Option<f64>| None;
    assert!(registration("/roll/b", &passes, (kr, kb), &none).is_none() && magnification(&frames, &none).is_none());
}

#[test]
fn the_normal_equations_solve_and_a_singular_system_says_so() {
    // 2x + y = 5, x − y = 1: x = 2, y = 1.
    let rows: Vec<Row> = vec![(vec![2.0, 1.0], 5.0, 1.0), (vec![1.0, -1.0], 1.0, 1.0)];
    let s = least_squares(&rows, 2).unwrap();
    assert!((s[0] - 2.0).abs() < 1e-12 && (s[1] - 1.0).abs() < 1e-12, "{s:?}");
    // Twin: one equation twice leaves the system singular.
    assert!(least_squares(&[(vec![1.0, 1.0], 2.0, 1.0), (vec![2.0, 2.0], 4.0, 1.0)], 2).is_none());
}
