//! The decode's own checks: the conversion, the routes and the depth of each file kind.
use super::*;

/// Adobe's matrix for the Nikon Z f, as LibRaw holds it.
const Z_F: M3 = [
    [1.1607, -0.4491, -0.0977],
    [-0.4522, 1.246, 0.2304],
    [-0.0458, 0.1519, 0.7616],
];

/// What LibRaw hands out for a white `w` whose most sensitive channel reaches the white level.
fn rendered(conv: &Conversion, w: [f64; 3]) -> [f64; 3] {
    let n = apply(&Z_F, w);
    let hi = n.iter().cloned().fold(0.0, f64::max);
    let top = conv.mul.iter().cloned().fold(0.0, f32::max) as f64;
    let o = [0, 1, 2].map(|c| n[c] / hi * conv.mul[c] as f64 / top);
    let m = conv.matrix.map(|r| r.map(f64::from));
    apply(&m, o)
}

#[test]
fn the_5000_k_white_reads_neutral_at_the_baseline() {
    let conv = converted_by(&Z_F, DEFAULT_BASELINE_EV, Colours::Matrix).unwrap();
    let level = DEFAULT_BASELINE_EV.exp2();
    let out = rendered(&conv, neutral_xyz());
    for v in out {
        assert!((v - level).abs() < 1e-5, "{out:?}");
    }
    // Twin: a D65 white, the balance of LibRaw's own pre_mul, comes out visibly blue.
    let d65 = rendered(&conv, xyz(D65_XY));
    assert!(d65[2] / d65[0] > 1.15, "{d65:?}");
}

/// True when `w` has the chromaticity of a 5000 K Planckian radiator, (0.3451, 0.3516).
fn planckian_5000(w: [f64; 3]) -> bool {
    let (x, y) = (w[0] / w.iter().sum::<f64>(), w[1] / w.iter().sum::<f64>());
    (x - 0.3451).abs() < 2e-4 && (y - 0.3516).abs() < 2e-4
}

#[test]
fn the_neutral_is_the_planckian_5000_k() {
    assert!(planckian_5000(neutral_xyz()), "{:?}", neutral_xyz());
    // Twin: D50 sits 0.007 above the locus in y and must not pass.
    assert!(!planckian_5000(xyz((0.3457, 0.3585))));
}

#[test]
fn rec2020_and_bradford_land_whites_on_one() {
    let d65 = xyz(D65_XY);
    let to_2020 = rec2020_from_xyz();
    for v in apply(&to_2020, d65) {
        assert!((v - 1.0).abs() < 1e-9);
    }
    let adapted = apply(&bradford(neutral_xyz(), d65), neutral_xyz());
    for c in 0..3 {
        assert!((adapted[c] - d65[c]).abs() < 1e-9);
    }
    // Twin: without adaptation the 5000 K white is not grey in Rec. 2020.
    let raw = apply(&to_2020, neutral_xyz());
    assert!(raw[0] / raw[2] > 1.2, "{raw:?}");
}

#[test]
fn a_dng_tag_replaces_the_default() {
    assert_eq!(baseline(-999.0), DEFAULT_BASELINE_EV);
    assert_eq!(baseline(0.35), 0.35f32 as f64);
    // Twin: a tag of zero is a value, not an absence.
    assert_ne!(baseline(0.0), DEFAULT_BASELINE_EV);
}

#[test]
fn the_extension_counts_only_when_the_bytes_claim_nothing() {
    let dir = std::env::temp_dir().join(format!("oni-route-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let jpeg = dir.join("scan.NEF");
    std::fs::write(&jpeg, b"not a container").unwrap();
    assert_eq!(route(&jpeg), Branch::Mosaic, "unclaimed bytes fall back on a RAW extension");
    let plain = dir.join("scan.tif");
    std::fs::write(&plain, b"not a container").unwrap();
    assert_eq!(route(&plain), Branch::Plain);
    // Twin: a missing file with a RAW name still sorts as RAW, and fails at the decode.
    assert!(decodes_via_mosaic(&dir.join("absent.rw2")));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The camera channels LibRaw hands out for XYZ `w` scaled so its brightest channel sits at `top`.
fn handed(conv: &Conversion, w: [f64; 3], top: f64) -> [f64; 3] {
    let n = apply(&Z_F, w);
    let hi = n.iter().cloned().fold(0.0, f64::max);
    let most = conv.mul.iter().cloned().fold(0.0, f32::max) as f64;
    let o = [0, 1, 2].map(|c| n[c] / hi * top * conv.mul[c] as f64 / most);
    apply(&conv.matrix.map(|r| r.map(f64::from)), o)
}

#[test]
fn the_sensor_s_channels_mix_nothing_and_its_neutral_keeps_the_matrix_s_level() {
    let stops = DEFAULT_BASELINE_EV;
    let (matrix, sensor) = (converted_by(&Z_F, stops, Colours::Matrix).unwrap(), converted_by(&Z_F, stops, Colours::Sensor).unwrap());
    assert_eq!(sensor.mul, matrix.mul, "LibRaw demosaics on the same multipliers");
    for r in 0..3 {
        for c in (0..3).filter(|&c| c != r) {
            assert_eq!(sensor.matrix[r][c], 0.0, "{:?}", sensor.matrix);
        }
    }
    let (a, b) = (handed(&matrix, neutral_xyz(), 1.0), handed(&sensor, neutral_xyz(), 1.0));
    for c in 0..3 {
        assert!((b[c] - stops.exp2()).abs() < 1e-5 && (a[c] - b[c]).abs() < 1e-5, "{a:?} {b:?}");
    }
    // Twin: a colour the matrix pushes apart; through it the red leaks into another channel.
    let red = handed(&matrix, xyz((0.5, 0.3)), 0.5);
    let red_sensor = handed(&sensor, xyz((0.5, 0.3)), 0.5);
    assert!(red.iter().zip(red_sensor).any(|(m, s)| (m - s).abs() > 0.05), "{red:?} {red_sensor:?}");
}

#[test]
fn the_unbalanced_sensor_reads_the_photosites_share_of_the_white_level() {
    let conv = converted_by(&Z_F, 0.0, Colours::SensorUnbalanced).unwrap();
    let n = apply(&Z_F, neutral_xyz());
    let hi = n.iter().cloned().fold(0.0, f64::max);
    let out = handed(&conv, neutral_xyz(), 1.0);
    for c in 0..3 {
        assert!((out[c] - n[c] / hi).abs() < 1e-5, "{out:?} for {n:?}");
    }
    // Twin: unbalanced, the 5000 K neutral is not grey.
    let spread = out.iter().cloned().fold(0.0, f64::max) - out.iter().cloned().fold(f64::MAX, f64::min);
    assert!(spread > 0.2, "{out:?}");
}

#[test]
fn a_singular_matrix_is_refused() {
    assert!(converted_by(&[[0.0; 3]; 3], 0.0, Colours::Sensor).is_none());
    // Twin: the Z f matrix is accepted.
    assert!(converted_by(&Z_F, 0.0, Colours::Sensor).is_some());
}
