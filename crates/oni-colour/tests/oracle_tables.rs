//! Every kind of table a paper profile carries, read against lcms2, and the whole export and
//! proof legs against lcms2's own black point compensation. Each agreement has a twin.

mod common;

use common::{Curve, assemble, legacy, multi};
use lcms2::{Flags, Intent, PixelFormat, Profile, Transform};
use moxcms::{ColorProfile, LutType, LutWarehouse};
use oni_colour::icc_lut::Table;
use oni_colour::legs::{self, Leg};
use oni_colour::pcs;
use oni_colour::profile;

const M: [[f64; 3]; 3] = [[0.4361, 0.3851, 0.1431], [0.2225, 0.7169, 0.0606], [0.0139, 0.0971, 0.7141]];
/// u1Fixed15: an encoded 1.0 is 1.99997 of XYZ.
const XYZ_SCALE: f64 = 65535.0 / 32768.0;
const LEGACY: f64 = 65280.0 / 65535.0;
/// A few times the measured agreement, which lcms2's 16-bit quantisation of a grid's input sets.
const XYZ_TOLERANCE: f32 = 3e-4;
const DEVICE_TOLERANCE: f32 = 1e-3;

fn printer(rgb: [f64; 3]) -> [f64; 3] {
    let shaped = rgb.map(|v| v.max(0.0).powf(1.15));
    std::array::from_fn(|i| 0.02 * [0.9642, 1.0, 0.8249][i] + 0.98 * (0..3).map(|j| M[i][j] * shaped[j]).sum::<f64>())
}

fn unprinter(xyz: [f64; 3]) -> [f64; 3] {
    let inv = pcs::inverse(&M).unwrap();
    let d: [f64; 3] = std::array::from_fn(|i| (xyz[i] - 0.02 * [0.9642, 1.0, 0.8249][i]) / 0.98);
    std::array::from_fn(|i| (0..3).map(|j| inv[i][j] * d[j]).sum::<f64>().clamp(0.0, 1.0).powf(1.0 / 1.15))
}

fn lab_v4(xyz: [f64; 3]) -> [f64; 3] {
    let l = pcs::lab(xyz.map(|v| v as f32));
    [l[0] as f64 / 100.0, (l[1] as f64 + 128.0) / 255.0, (l[2] as f64 + 128.0) / 255.0]
}

fn from_v4(e: [f64; 3]) -> [f64; 3] {
    pcs::xyz_from_lab([(e[0] * 100.0) as f32, (e[1] * 255.0 - 128.0) as f32, (e[2] * 255.0 - 128.0) as f32]).map(|v| v as f64)
}

fn power(p: f64) -> Curve {
    Curve::Sampled((0..256).map(|i| (i as f64 / 255.0).powf(p)).collect())
}

/// Device colours: a 7-step grid and a dense shadow ramp.
fn device_samples() -> Vec<[f32; 3]> {
    let mut v = vec![];
    for r in 0..7 {
        for g in 0..7 {
            for b in 0..7 {
                v.push([r as f32 / 6.0, g as f32 / 6.0, b as f32 / 6.0]);
            }
        }
    }
    for i in 0..40 {
        let t = (i as f32 / 39.0).powi(3) * 0.2;
        v.push([t, t * 0.8, t * 1.1]);
    }
    v
}

fn rgba(v: &[[f32; 3]]) -> Vec<f32> {
    v.iter().flat_map(|p| [p[0], p[1], p[2], 1.0]).collect()
}

fn rgb(v: &[f32]) -> Vec<[f32; 3]> {
    v.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect()
}

fn gap(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).flat_map(|(x, y)| (0..3).map(move |c| (x[c] - y[c]).abs())).fold(0.0, f32::max)
}

fn lcms(from: &Profile, fin: PixelFormat, to: &Profile, fout: PixelFormat, input: &[[f32; 3]], bpc: bool) -> Vec<[f32; 3]> {
    let flags = if bpc { Flags::NO_OPTIMIZE | Flags::BLACKPOINT_COMPENSATION } else { Flags::NO_OPTIMIZE };
    let t: Transform<[f32; 3], [f32; 3]> = Transform::new_flags(from, fin, to, fout, Intent::RelativeColorimetric, flags).unwrap();
    let mut out = vec![[0.0; 3]; input.len()];
    t.transform_pixels(input, &mut out);
    out
}

/// The profile's two tables read by both CMMs; returns the A2B and B2A gaps and our parse.
fn read_both(data: &[u8]) -> (f32, f32, ColorProfile, Vec<[f32; 3]>) {
    let ours = profile::parse(data).expect("moxcms parses the profile");
    let theirs = Profile::new_icc(data).expect("lcms2 parses the profile");
    let xyz = Profile::new_xyz();
    let device = device_samples();
    let read_l = lcms(&theirs, PixelFormat::RGB_FLT, &xyz, PixelFormat::XYZ_FLT, &device, false);
    let read_o = rgb(&Leg::to_xyz(&ours).unwrap().xyz(&rgba(&device)).unwrap());
    let back_l = lcms(&xyz, PixelFormat::XYZ_FLT, &theirs, PixelFormat::RGB_FLT, &read_l, false);
    let back_o = rgb(&Leg::from_xyz(&ours).unwrap().device(&rgba(&read_l)).unwrap());
    (gap(&read_l, &read_o), gap(&back_l, &back_o), ours, read_l)
}

/// The A2B read through a table edited by `twin`, set against lcms2's reading of the true one.
fn a2b_twin(ours: &ColorProfile, read_l: &[[f32; 3]], twin: impl Fn(&mut LutWarehouse)) -> f32 {
    let mut table = ours.lut_a_to_b_colorimetric.clone().unwrap();
    twin(&mut table);
    let wrong = Table::new(&table, ours.pcs, true).unwrap();
    let read: Vec<[f32; 3]> = device_samples().iter().map(|d| wrong.xyz(*d)).collect();
    gap(read_l, &read)
}

fn b2a_twin(ours: &ColorProfile, read_l: &[[f32; 3]], back_l: &[[f32; 3]], twin: impl Fn(&mut LutWarehouse)) -> f32 {
    let mut table = ours.lut_b_to_a_colorimetric.clone().unwrap();
    twin(&mut table);
    let wrong = Table::new(&table, ours.pcs, false).unwrap();
    let back: Vec<[f32; 3]> = read_l.iter().map(|x| wrong.device(*x)).collect();
    gap(back_l, &back)
}

fn lcms_back(data: &[u8], read_l: &[[f32; 3]]) -> Vec<[f32; 3]> {
    lcms(&Profile::new_xyz(), PixelFormat::XYZ_FLT, &Profile::new_icc(data).unwrap(), PixelFormat::RGB_FLT, read_l, false)
}

#[test]
fn a_v2_lab_lut16_paper_reads_as_lcms2_reads_it() {
    // The common paper profile: ICC v2, Lab PCS, lut16 tables in the legacy Lab encoding.
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let a2b = legacy(16, identity, &|_, t| t.powf(0.8), 17, &|v| lab_v4(printer(v.map(|x| x.powf(1.25)))).map(|e| e * LEGACY), &|_, t| t);
    let b2a = legacy(16, identity, &|_, t| t, 33, &|e| unprinter(from_v4(e.map(|x| x / LEGACY))), &|c, t| t.powf(1.0 + 0.05 * c as f64));
    let data = assemble("Lab lut16", 2, true, None, vec![("A2B0", a2b), ("B2A0", b2a)]);
    let (a, b, ours, read_l) = read_both(&data);
    assert!(a < XYZ_TOLERANCE && b < DEVICE_TOLERANCE, "A2B {a}, B2A {b}");
    // Twin: the same tables read in the v4 Lab encoding, as an 8-bit table would be.
    let legacy_as_v4 = |t: &mut LutWarehouse| {
        if let LutWarehouse::Lut(l) = t {
            l.lut_type = LutType::Lut8;
        }
    };
    let wrong_a = a2b_twin(&ours, &read_l, legacy_as_v4);
    let wrong_b = b2a_twin(&ours, &read_l, &lcms_back(&data, &read_l), legacy_as_v4);
    assert!(wrong_a > 10.0 * XYZ_TOLERANCE && wrong_b > 3.0 * DEVICE_TOLERANCE, "{wrong_a} {wrong_b}");
}

#[test]
fn a_v2_lab_lut8_paper_reads_as_lcms2_reads_it() {
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let a2b = legacy(8, identity, &|_, t| t, 9, &|v| lab_v4(printer(v)), &|_, t| t);
    let b2a = legacy(8, identity, &|_, t| t, 17, &|e| unprinter(from_v4(e)), &|_, t| t);
    let data = assemble("Lab lut8", 2, true, None, vec![("A2B0", a2b), ("B2A0", b2a)]);
    let (a, b, ours, read_l) = read_both(&data);
    // Eight-bit nodes: lcms2 and this reading round them alike, so the gap stays at interpolation.
    assert!(a < XYZ_TOLERANCE && b < DEVICE_TOLERANCE, "A2B {a}, B2A {b}");
    // Twin: the same 8-bit tables read in the legacy 16-bit Lab encoding.
    let as_legacy = |t: &mut LutWarehouse| {
        if let LutWarehouse::Lut(l) = t {
            l.lut_type = LutType::Lut16;
        }
    };
    let wrong = a2b_twin(&ours, &read_l, as_legacy);
    assert!(wrong > 10.0 * XYZ_TOLERANCE, "{wrong}");
}

#[test]
fn a_lut16_b2a_on_xyz_lays_its_matrix_first() {
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let k = [[0.92, 0.05, 0.03], [0.04, 0.9, 0.06], [0.02, 0.07, 0.91]];
    let kinv = pcs::inverse(&k).unwrap();
    let a2b = legacy(16, identity, &|_, t| t, 17, &|v| printer(v).map(|x| x / XYZ_SCALE), &|_, t| t);
    let b2a = legacy(16, k, &|_, t| t.sqrt(), 33, &|e| {
        let undone = pcs::mul(&kinv, e.map(|x| x * x));
        unprinter(undone.map(|x| x * XYZ_SCALE))
    }, &|_, t| t);
    let data = assemble("XYZ matrix", 2, false, None, vec![("A2B0", a2b), ("B2A0", b2a)]);
    let (a, b, ours, read_l) = read_both(&data);
    assert!(a < XYZ_TOLERANCE && b < DEVICE_TOLERANCE, "A2B {a}, B2A {b}");
    // Twin: the matrix left out.
    let wrong = b2a_twin(&ours, &read_l, &lcms_back(&data, &read_l), |t| {
        if let LutWarehouse::Lut(l) = t {
            l.matrix.v = identity;
        }
    });
    assert!(wrong > 3.0 * DEVICE_TOLERANCE, "{wrong}");
}

fn swap_outer_curves(t: &mut LutWarehouse) {
    if let LutWarehouse::Multidimensional(m) = t {
        std::mem::swap(&mut m.a_curves, &mut m.b_curves);
    }
}

/// `mAB` and `mBA` with all five elements, on an XYZ or a Lab PCS, the B2A grid in 8 bits.
fn v4_profile(lab: bool) -> Vec<u8> {
    let k = [[0.9, 0.06, 0.04], [0.05, 0.88, 0.07], [0.03, 0.05, 0.92]];
    let bias = [0.01, 0.0, 0.02];
    let srgb = Curve::Para(3, vec![2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045]);
    let srgb2 = Curve::Para(3, vec![2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045]);
    let srgb3 = Curve::Para(3, vec![2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045]);
    let encode = move |xyz: [f64; 3]| if lab { lab_v4(xyz) } else { xyz.map(|x| x / XYZ_SCALE) };
    let decode = move |e: [f64; 3]| if lab { from_v4(e) } else { e.map(|x| x * XYZ_SCALE) };
    let a2b_grid = move |v: [f64; 3]| -> [f64; 3] { encode(printer(v)).map(|x| (x - 0.02).clamp(0.0, 1.0) * 0.9) };
    let a2b = multi(
        "mAB ",
        [power(0.9), power(0.95), power(1.0)],
        Some((k, bias)),
        Some([srgb, Curve::Gamma(1.2), power(1.1)]),
        Some((9, 2, &a2b_grid)),
        Some([Curve::Gamma(1.8), Curve::Gamma(1.6), Curve::Para(0, vec![1.4])]),
    );
    let b2a_grid = move |e: [f64; 3]| -> [f64; 3] { unprinter(decode(e)) };
    let b2a = multi(
        "mBA ",
        [Curve::Para(0, vec![1.1]), power(1.05), srgb2],
        Some((k, [0.0, 0.01, 0.0])),
        Some([power(0.95), Curve::Gamma(1.1), srgb3]),
        Some((11, 1, &b2a_grid)),
        Some([Curve::Gamma(0.8), power(0.9), Curve::Gamma(1.0)]),
    );
    assemble(if lab { "v4 Lab" } else { "v4 XYZ" }, 4, lab, None, vec![("A2B0", a2b), ("B2A0", b2a)])
}

#[test]
fn v4_tables_run_their_five_elements_in_the_icc_order() {
    for lab in [false, true] {
        let data = v4_profile(lab);
        let (a, b, ours, read_l) = read_both(&data);
        assert!(a < XYZ_TOLERANCE && b < DEVICE_TOLERANCE, "Lab {lab}: A2B {a}, B2A {b}");
        // Twin: the A and B curves read at each other's end of the chain.
        let wrong_a = a2b_twin(&ours, &read_l, swap_outer_curves);
        let wrong_b = b2a_twin(&ours, &read_l, &lcms_back(&data, &read_l), swap_outer_curves);
        assert!(wrong_a > 10.0 * XYZ_TOLERANCE && wrong_b > 3.0 * DEVICE_TOLERANCE, "Lab {lab}: {wrong_a} {wrong_b}");
    }
}

/// Working-space colours, in gamut and past every printer's: the export's own inputs.
fn working_samples() -> Vec<[f32; 3]> {
    let mut v = vec![];
    for r in 0..6 {
        for g in 0..6 {
            for b in 0..6 {
                v.push([r as f32 / 5.0, g as f32 / 5.0, b as f32 / 5.0].map(|x| x * x));
            }
        }
    }
    for i in 0..30 {
        let t = (i as f32 / 29.0).powi(3) * 0.05;
        v.push([t; 3]);
    }
    v.extend([[0.02, 0.6, 0.1], [0.9, 0.02, 0.1], [0.05, 0.05, 0.8], [1.2, 1.1, 1.0]]);
    v
}

fn lcms_working() -> Profile {
    Profile::new_icc(&oni_colour::working::working_profile().encode().unwrap()).unwrap()
}

#[test]
fn the_export_legs_agree_with_lcms2_s_black_point_compensation() {
    for lab_v4 in [false, true] {
        // The colorimetric tags laid too: without them lcms2 finds no black point to compensate.
        let mut tables = common::stand_in_tables(0.03, lab_v4, 17, 33);
        tables.extend(tables.clone().into_iter().map(|(sig, t)| (if sig == "A2B0" { "A2B1" } else { "B2A1" }, t)));
        let data = assemble("BPC", if lab_v4 { 4 } else { 2 }, lab_v4, None, tables);
        let theirs = Profile::new_icc(&data).unwrap();
        let legs = legs::legs(&data).unwrap();
        let samples = working_samples();
        let ours = rgb(&legs::run(&legs, &rgba(&samples)).unwrap());
        let clip = |v: Vec<[f32; 3]>| -> Vec<[f32; 3]> { v.into_iter().map(|p| p.map(|x| x.clamp(0.0, 1.0))).collect() };
        let l = clip(lcms(&lcms_working(), PixelFormat::RGB_FLT, &theirs, PixelFormat::RGB_FLT, &samples, true));
        let g = gap(&l, &ours);
        assert!(g < 2e-3, "v4 {lab_v4}: export {g}");
        // Twin: the same conversion without compensation sits the shadows on the paper's black.
        let bare = clip(lcms(&lcms_working(), PixelFormat::RGB_FLT, &theirs, PixelFormat::RGB_FLT, &samples, false));
        assert!(gap(&bare, &ours) > 0.02, "v4 {lab_v4}: {}", gap(&bare, &ours));

        // Colour only reads the device values back with the compensation undone.
        let device = rgb(&legs::run(&legs, &rgba(&samples)).unwrap());
        let back = legs::legs_back(&data).unwrap();
        let shown = rgb(&legs::run_back(&back, &rgba(&device), Some(&legs)).unwrap());
        let l = lcms(&theirs, PixelFormat::RGB_FLT, &lcms_working(), PixelFormat::RGB_FLT, &device, true);
        let g = gap(&l, &shown);
        assert!(g < 2e-3, "v4 {lab_v4}: colour only {g}");
        // Twin: the proof at rest keeps the paper's black.
        let rest = rgb(&legs::run_back(&back, &rgba(&device), None).unwrap());
        assert!(gap(&l, &rest) > 0.02, "v4 {lab_v4}: {}", gap(&l, &rest));
    }
}
