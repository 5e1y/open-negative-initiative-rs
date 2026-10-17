//! The legs against lcms2, the oracle, through generated profiles. Each agreement has a twin.

mod common;

use lcms2::{CIExyY, CIExyYTRIPLE, Flags, Intent, PixelFormat, Profile, ToneCurve, Transform};
use moxcms::Layout;
use oni_colour::legs::{Leg, relative};
use oni_colour::profile;
use oni_colour::space::Space;
use oni_colour::working::PCS_PROFILE;

fn lcms(from: &Profile, fin: PixelFormat, to: &Profile, fout: PixelFormat, input: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let t: Transform<[f32; 3], [f32; 3]> =
        Transform::new_flags(from, fin, to, fout, Intent::RelativeColorimetric, Flags::NO_OPTIMIZE).unwrap();
    let mut out = vec![[0.0; 3]; input.len()];
    t.transform_pixels(input, &mut out);
    out
}

fn rgba(v: &[[f32; 3]]) -> Vec<f32> {
    v.iter().flat_map(|p| [p[0], p[1], p[2], 1.0]).collect()
}

fn rgb(v: &[f32]) -> Vec<[f32; 3]> {
    v.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect()
}

fn clipped(v: Vec<[f32; 3]>) -> Vec<[f32; 3]> {
    v.into_iter().map(|p| p.map(|x| x.clamp(0.0, 1.0))).collect()
}

fn gap(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
    a.iter().zip(b).flat_map(|(x, y)| (0..3).map(move |c| (x[c] - y[c]).abs())).fold(0.0, f32::max)
}

/// Device colours: a 9-step grid and a dense shadow ramp.
fn device_samples() -> Vec<[f32; 3]> {
    let mut v = vec![];
    for r in 0..9 {
        for g in 0..9 {
            for b in 0..9 {
                v.push([r as f32 / 8.0, g as f32 / 8.0, b as f32 / 8.0]);
            }
        }
    }
    for i in 0..40 {
        let t = (i as f32 / 39.0).powi(3) * 0.2;
        v.push([t, t * 0.8, t * 1.1]);
    }
    v
}

/// The working space as lcms2 builds it from its chromaticities: Rec. 2020 primaries, D65, linear.
fn lcms_working_from_xy() -> Profile {
    let white = CIExyY { x: 0.3127, y: 0.3290, Y: 1.0 };
    let primaries = CIExyYTRIPLE {
        Red: CIExyY { x: 0.708, y: 0.292, Y: 1.0 },
        Green: CIExyY { x: 0.170, y: 0.797, Y: 1.0 },
        Blue: CIExyY { x: 0.131, y: 0.046, Y: 1.0 },
    };
    let linear = ToneCurve::new(1.0);
    Profile::new_rgb(&white, &primaries, &[&linear, &linear, &linear]).unwrap()
}

/// A few times the measured agreement: 3.7e-5 in XYZ, 1.2e-4 in device values.
const XYZ_TOLERANCE: f32 = 2e-4;
const DEVICE_TOLERANCE: f32 = 5e-4;

#[test]
fn a_paper_s_tables_read_as_lcms2_reads_them() {
    for lab_v4 in [false, true] {
        let data = common::stand_in("Oracle", 0.02, None, lab_v4);
        let ours = profile::parse(&data).unwrap();
        let theirs = Profile::new_icc(&data).unwrap();
        let xyz = Profile::new_xyz();
        let device = device_samples();

        let read_l = lcms(&theirs, PixelFormat::RGB_FLT, &xyz, PixelFormat::XYZ_FLT, &device);
        let read_o = rgb(&Leg::to_xyz(&ours).unwrap().xyz(&rgba(&device)).unwrap());
        let a2b = gap(&read_l, &read_o);
        assert!(a2b < XYZ_TOLERANCE, "v4 {lab_v4}: A2B {a2b}");

        let back_l = lcms(&xyz, PixelFormat::XYZ_FLT, &theirs, PixelFormat::RGB_FLT, &read_l);
        let back_o = rgb(&Leg::from_xyz(&ours).unwrap().device(&rgba(&read_l)).unwrap());
        let b2a = gap(&back_l, &back_o);
        assert!(b2a < DEVICE_TOLERANCE, "v4 {lab_v4}: B2A {b2a}");

        // Twin: the same B2A through a CMM that bakes the transform into one grid, interpolating
        // twice, which this reading avoids.
        let baked = PCS_PROFILE.create_transform_f32(Layout::Rgba, &ours, Layout::Rgba, relative()).unwrap();
        let mut out = vec![0.0; device.len() * 4];
        baked.transform(&rgba(&read_l), &mut out).unwrap();
        let twin = gap(&back_l, &rgb(&out));
        assert!(twin > 20.0 * DEVICE_TOLERANCE, "v4 {lab_v4}: baked B2A {twin}");
    }
}

#[test]
fn a_coarse_grid_is_read_between_its_own_nodes_as_lcms2_reads_it() {
    // A 5-node Lab A2B: a grey halfway between two nodes is read off the grey axis alone.
    let data = common::stand_in("Coarse", 0.02, None, true);
    let ours = profile::parse(&data).unwrap();
    let theirs = Profile::new_icc(&data).unwrap();
    let grey = [[0.125f32; 3]];
    let l = lcms(&theirs, PixelFormat::RGB_FLT, &Profile::new_xyz(), PixelFormat::XYZ_FLT, &grey);
    let o = rgb(&Leg::to_xyz(&ours).unwrap().xyz(&rgba(&grey)).unwrap());
    assert!(gap(&l, &o) < XYZ_TOLERANCE, "{l:?} {o:?}");
    // Twin: trilinear interpolation, which mixes the chromatic corners in, reads it brighter.
    let trilinear = ours.create_transform_f32(Layout::Rgba, &PCS_PROFILE, Layout::Rgba, relative()).unwrap();
    let mut out = [0.0f32; 4];
    trilinear.transform(&rgba(&grey), &mut out).unwrap();
    assert!((out[1] - l[0][1]).abs() > 0.01, "trilinear {out:?} against {l:?}");
}

/// The working space as this crate defines it, for lcms2 to convert from.
fn lcms_working() -> Profile {
    Profile::new_icc(&oni_colour::working::working_profile().encode().unwrap()).unwrap()
}

#[test]
fn the_working_space_is_rec2020_on_the_pcs() {
    let theirs = lcms_working_from_xy();
    let unit = [[1.0f32, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0, 1.0]];
    let l = lcms(&theirs, PixelFormat::RGB_FLT, &Profile::new_xyz(), PixelFormat::XYZ_FLT, &unit);
    let o: Vec<[f32; 3]> = unit.iter().map(|p| oni_colour::pcs::mul_f32(&oni_colour::working::WORKING_TO_PCS, *p)).collect();
    assert!(gap(&l, &o) < 2e-4, "{l:?} {o:?}");
    // Twin: the sRGB primaries put the red elsewhere.
    let srgb = lcms(&Profile::new_srgb(), PixelFormat::RGB_FLT, &Profile::new_xyz(), PixelFormat::XYZ_FLT, &unit);
    assert!(gap(&srgb, &o) > 0.05);
}

#[test]
fn the_matrix_spaces_encode_as_lcms2_converts() {
    let working = lcms_working();
    let mut samples = device_samples();
    samples.extend([[0.02, 0.6, 0.1], [0.9, 0.02, 0.1], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0], [0.18, 0.18, 0.18]]);
    for space in Space::BUILT_IN {
        let theirs = Profile::new_icc(&space.icc_data().unwrap()).unwrap();
        let l = clipped(lcms(&working, PixelFormat::RGB_FLT, &theirs, PixelFormat::RGB_FLT, &samples));
        let o = rgb(&space.encoded(&rgba(&samples)).unwrap());
        let g = gap(&l, &o);
        assert!(g < 5e-4, "{}: {g}", space.title());
    }
    let srgb = rgb(&Space::Srgb.encoded(&rgba(&samples)).unwrap());
    let own = clipped(lcms(&working, PixelFormat::RGB_FLT, &Profile::new_srgb(), PixelFormat::RGB_FLT, &samples));
    // Two sRGB definitions, rounded apart, part by 4e-3 of code where a channel cancels to black.
    assert!(gap(&own, &srgb) < 5e-3, "lcms2's own sRGB: {}", gap(&own, &srgb));
    // Twin: the sRGB encoding set against Display P3 as lcms2 converts it.
    let p3 = Profile::new_icc(&Space::DisplayP3.icc_data().unwrap()).unwrap();
    let other = clipped(lcms(&working, PixelFormat::RGB_FLT, &p3, PixelFormat::RGB_FLT, &samples));
    assert!(gap(&other, &srgb) > 0.05);
}

#[test]
fn a_matrix_profile_leg_agrees_with_lcms2() {
    let adobe = Space::AdobeRgb.icc_data().unwrap();
    let ours = profile::parse(&adobe).unwrap();
    let theirs = Profile::new_icc(&adobe).unwrap();
    let device = device_samples();
    let xyz = Profile::new_xyz();
    let l = lcms(&theirs, PixelFormat::RGB_FLT, &xyz, PixelFormat::XYZ_FLT, &device);
    let o = rgb(&Leg::to_xyz(&ours).unwrap().xyz(&rgba(&device)).unwrap());
    assert!(gap(&l, &o) < XYZ_TOLERANCE, "to XYZ {}", gap(&l, &o));
    let d = rgb(&Leg::from_xyz(&ours).unwrap().device(&rgba(&l)).unwrap());
    let back = lcms(&xyz, PixelFormat::XYZ_FLT, &theirs, PixelFormat::RGB_FLT, &l);
    // Read in light: a pure power curve's slope at black turns 1e-7 of light into 5e-4 of code.
    let light = |v: Vec<[f32; 3]>| -> Vec<[f32; 3]> { v.into_iter().map(|p| p.map(|x| x.max(0.0).powf(563.0 / 256.0))).collect() };
    let g = gap(&light(back), &light(d));
    assert!(g < XYZ_TOLERANCE, "from XYZ {g}");
    // Twin: the same values read through sRGB's leg.
    let srgb = profile::parse(&Space::Srgb.icc_data().unwrap()).unwrap();
    let s = rgb(&Leg::to_xyz(&srgb).unwrap().xyz(&rgba(&device)).unwrap());
    assert!(gap(&l, &s) > 0.01);
}
