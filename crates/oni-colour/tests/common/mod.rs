//! Stand-in printer profiles, written byte by byte, since no paper profile is shipped.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use oni_colour::pcs;

const M: [[f64; 3]; 3] = [[0.4361, 0.3851, 0.1431], [0.2225, 0.7169, 0.0606], [0.0139, 0.0971, 0.7141]];
const WHITE: [f64; 3] = [0.9642, 1.0, 0.8249];

fn u16b(v: i64) -> [u8; 2] {
    [((v >> 8) & 0xFF) as u8, (v & 0xFF) as u8]
}
fn u32b(v: i64) -> Vec<u8> {
    [u16b(v >> 16), u16b(v & 0xFFFF)].concat()
}
fn s15(v: f64) -> Vec<u8> {
    u32b(((v * 65536.0).round() as i64) & 0xFFFF_FFFF)
}
fn pad(mut b: Vec<u8>) -> Vec<u8> {
    while !b.len().is_multiple_of(4) {
        b.push(0);
    }
    b
}

/// A real printer profile for checks: ICC v2 RGB to XYZ, or v4 RGB to Lab with `lab_v4`. Its
/// device black prints `black` of the white, and `paper` names the media white.
pub fn stand_in(name: &str, black: f64, paper: Option<[f64; 3]>, lab_v4: bool) -> Vec<u8> {
    let (a2b, b2a) = if lab_v4 { (5, 17) } else { (2, 17) };
    stand_in_grids(name, black, paper, lab_v4, a2b, b2a)
}

/// The same with the tables' grid sizes chosen, finer than a check's stand-in where two CMMs'
/// interpolation is compared rather than their reading of the tags.
pub fn stand_in_grids(name: &str, black: f64, paper: Option<[f64; 3]>, lab_v4: bool, a2b: usize, b2a: usize) -> Vec<u8> {
    assemble(name, if lab_v4 { 4 } else { 2 }, lab_v4, paper, stand_in_tables(black, lab_v4, a2b, b2a))
}

/// The stand-in's `A2B0` and `B2A0`.
pub fn stand_in_tables(black: f64, lab_v4: bool, a2b: usize, b2a: usize) -> Vec<(&'static str, Vec<u8>)> {
    let xyz = |rgb: [f64; 3]| -> [f64; 3] {
        std::array::from_fn(|i| black * WHITE[i] + (1.0 - black) * (0..3).map(|j| M[i][j] * rgb[j]).sum::<f64>())
    };
    let inv = pcs::inverse(&M).unwrap();
    let inverse = |x: [f64; 3]| -> [f64; 3] {
        let d: [f64; 3] = std::array::from_fn(|i| (x[i] - black * WHITE[i]) / (1.0 - black));
        std::array::from_fn(|i| (0..3).map(|j| inv[i][j] * d[j]).sum::<f64>().clamp(0.0, 1.0))
    };
    // `mft2`: identity matrix, curves around a grid, first input varying slowest. `shaped` lays a
    // cube-root input curve, so the grid crowds the shadows as a real one does.
    let mft2 = |grid: usize, f: &dyn Fn([f64; 3]) -> [f64; 3], in_scale: f64, out_scale: f64, shaped: bool| -> Vec<u8> {
        let mut b: Vec<u8> = [b"mft2".to_vec(), u32b(0), vec![3, 3, grid as u8, 0]].concat();
        for i in 0..9 {
            b.extend(s15(if i % 4 == 0 { 1.0 } else { 0.0 }));
        }
        let entries = if shaped { 256 } else { 2 };
        b.extend(u16b(entries));
        b.extend(u16b(2));
        for _ in 0..3 {
            for i in 0..entries {
                let t = i as f64 / (entries - 1) as f64;
                b.extend(u16b(((if shaped { t.powf(1.0 / 3.0) } else { t }) * 65535.0).round() as i64));
            }
        }
        for r in 0..grid {
            for g in 0..grid {
                for bl in 0..grid {
                    let v = [r, g, bl].map(|k| {
                        let t = k as f64 / (grid - 1) as f64;
                        (if shaped { t * t * t } else { t }) * in_scale
                    });
                    for out in f(v) {
                        b.extend(u16b(((out / out_scale).clamp(0.0, 1.0) * 65535.0).round() as i64));
                    }
                }
            }
        }
        for _ in 0..3 {
            b.extend(u16b(0));
            b.extend(u16b(65535));
        }
        b
    };
    // v4 Lab tables: identity curves around a 16-bit grid, the neutral at 0x8080.
    let lab_of = |x: [f64; 3]| -> [f64; 3] {
        let l = pcs::lab([x[0] as f32, x[1] as f32, x[2] as f32]);
        [l[0] as f64 / 100.0, (l[1] as f64 + 128.0) / 255.0, (l[2] as f64 + 128.0) / 255.0]
    };
    let xyz_of = |e: [f64; 3]| -> [f64; 3] {
        let x = pcs::xyz_from_lab([(e[0] * 100.0) as f32, (e[1] * 255.0 - 128.0) as f32, (e[2] * 255.0 - 128.0) as f32]);
        x.map(|v| v as f64)
    };
    let lut = |tag: &str, grid: usize, f: &dyn Fn([f64; 3]) -> [f64; 3]| -> Vec<u8> {
        let identity: Vec<u8> = [b"curv".to_vec(), u32b(0), u32b(0)].concat();
        let mut clut: Vec<u8> = [vec![grid as u8; 3], vec![0; 13], vec![2, 0, 0, 0]].concat();
        for i in 0..grid {
            for j in 0..grid {
                for k in 0..grid {
                    let v = [i, j, k].map(|n| n as f64 / (grid - 1) as f64);
                    for out in f(v) {
                        clut.extend(u16b((out.clamp(0.0, 1.0) * 65535.0).round() as i64));
                    }
                }
            }
        }
        let clut = pad(clut);
        let curves = [identity.clone(), identity.clone(), identity].concat();
        let n = curves.len() as i64;
        let offsets = [u32b(32), u32b(0), u32b(0), u32b(32 + n), u32b(32 + n + clut.len() as i64)].concat();
        [tag.as_bytes().to_vec(), u32b(0), vec![3, 3, 0, 0], offsets, curves.clone(), clut, curves].concat()
    };
    let xyz_scale = 65535.0 / 32768.0;
    if lab_v4 {
        vec![("A2B0", lut("mAB ", a2b, &|v| lab_of(xyz(v)))), ("B2A0", lut("mBA ", b2a, &|v| inverse(xyz_of(v))))]
    } else {
        vec![("A2B0", mft2(a2b, &xyz, 1.0, xyz_scale, false)), ("B2A0", mft2(b2a, &inverse, xyz_scale, 1.0, true))]
    }
}

/// A printer profile around the given tags: header, `desc` (`mluc` on v4), `wtpt`, `cprt`.
pub fn assemble(name: &str, version: u8, lab: bool, paper: Option<[f64; 3]>, tables: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let ascii = name.as_bytes().to_vec();
    let utf16: Vec<u8> = name.encode_utf16().flat_map(|c| u16b(c as i64)).collect();
    let record = [u32b(1), u32b(12), b"enUS".to_vec(), u32b(utf16.len() as i64), u32b(28)].concat();
    let mluc = [b"mluc".to_vec(), u32b(0), record, utf16].concat();
    let legacy = [b"desc".to_vec(), u32b(0), u32b(ascii.len() as i64 + 1), ascii, vec![0], u32b(0), u32b(0), u16b(0).to_vec(), vec![0], vec![0; 67]].concat();
    let white_tag: Vec<u8> = [b"XYZ ".to_vec(), u32b(0), paper.unwrap_or(WHITE).iter().flat_map(|v| s15(*v)).collect()].concat();
    let cprt = [b"text".to_vec(), u32b(0), b"No copyright, check fixture".to_vec(), vec![0]].concat();
    let mut tags = vec![("desc", if version >= 4 { mluc } else { legacy }), ("wtpt", white_tag), ("cprt", cprt)];
    tags.extend(tables);
    let mut body = vec![];
    let mut table = u32b(tags.len() as i64);
    let mut offset = 128 + 4 + 12 * tags.len() as i64;
    for (sig, data) in &tags {
        table.extend(sig.as_bytes());
        table.extend(u32b(offset));
        table.extend(u32b(data.len() as i64));
        let padded = pad(data.clone());
        offset += padded.len() as i64;
        body.extend(padded);
    }
    let mut header = [u32b(0), u32b(0), if version >= 4 { vec![4, 0x30, 0, 0] } else { vec![2, 0x10, 0, 0] }].concat();
    header.extend(if lab { b"prtrRGB Lab " } else { b"prtrRGB XYZ " });
    header.extend(vec![0; 12]);
    header.extend(b"acspAPPL");
    header.extend(vec![0; 4 + 4 + 4 + 8 + 4]);
    header.extend(WHITE.iter().flat_map(|v| s15(*v)));
    header.resize(128, 0);
    let mut all = [header, table, body].concat();
    let size = u32b(all.len() as i64);
    all[0..4].copy_from_slice(&size);
    all
}

/// One curve of a table: a `curv` power, a sampled `curv`, or a `para` of the given ICC type.
pub enum Curve {
    Gamma(f64),
    Sampled(Vec<f64>),
    Para(u16, Vec<f64>),
}

fn curve_bytes(curve: &Curve) -> Vec<u8> {
    pad(match curve {
        Curve::Gamma(g) => [b"curv".to_vec(), u32b(0), u32b(1), u16b((g * 256.0).round() as i64).to_vec()].concat(),
        Curve::Sampled(v) => {
            let samples: Vec<u8> = v.iter().flat_map(|x| u16b((x.clamp(0.0, 1.0) * 65535.0).round() as i64)).collect();
            [b"curv".to_vec(), u32b(0), u32b(v.len() as i64), samples].concat()
        }
        Curve::Para(kind, p) => {
            [b"para".to_vec(), u32b(0), u16b(*kind as i64).to_vec(), u16b(0).to_vec(), p.iter().flat_map(|x| s15(*x)).collect()].concat()
        }
    })
}

/// A grid of `side` nodes a side, first input slowest, 8 or 16 bits a value.
fn grid_values(side: usize, bytes: u8, f: &dyn Fn([f64; 3]) -> [f64; 3]) -> Vec<u8> {
    let mut out = vec![];
    for i in 0..side {
        for j in 0..side {
            for k in 0..side {
                let v = [i, j, k].map(|n| n as f64 / (side - 1) as f64);
                for x in f(v) {
                    let x = x.clamp(0.0, 1.0);
                    if bytes == 1 {
                        out.push((x * 255.0).round() as u8);
                    } else {
                        out.extend(u16b((x * 65535.0).round() as i64));
                    }
                }
            }
        }
    }
    out
}

/// A grid's side, its bytes a value and the function its nodes sample.
pub type Grid<'a> = (usize, u8, &'a dyn Fn([f64; 3]) -> [f64; 3]);

/// A `lutAtoBType` (`mAB `) or `lutBtoAType` (`mBA `) with every element it may carry.
pub fn multi(tag: &str, b: [Curve; 3], matrix: Option<([[f64; 3]; 3], [f64; 3])>, m: Option<[Curve; 3]>, clut: Option<Grid>, a: Option<[Curve; 3]>) -> Vec<u8> {
    let curves = |set: &[Curve; 3]| set.iter().flat_map(curve_bytes).collect::<Vec<u8>>();
    let mut body = vec![];
    let mut place = |bytes: Vec<u8>| -> i64 {
        let at = 32 + body.len() as i64;
        body.extend(pad(bytes));
        at
    };
    let b_at = place(curves(&b));
    let matrix_at = matrix.map_or(0, |(mx, bias)| place(mx.iter().flatten().chain(bias.iter()).flat_map(|v| s15(*v)).collect()));
    let m_at = m.as_ref().map_or(0, |set| place(curves(set)));
    let clut_at = clut.map_or(0, |(side, bytes, f)| place([vec![side as u8; 3], vec![0; 13], vec![bytes, 0, 0, 0], grid_values(side, bytes, f)].concat()));
    let a_at = a.as_ref().map_or(0, |set| place(curves(set)));
    let offsets = [b_at, matrix_at, m_at, clut_at, a_at].iter().flat_map(|v| u32b(*v)).collect::<Vec<u8>>();
    [tag.as_bytes().to_vec(), u32b(0), vec![3, 3, 0, 0], offsets, body].concat()
}

/// A `lut8Type` (`bits` 8) or `lut16Type` (16): matrix, input tables, grid, output tables.
pub fn legacy(bits: u8, matrix: [[f64; 3]; 3], input: &dyn Fn(usize, f64) -> f64, side: usize, f: &dyn Fn([f64; 3]) -> [f64; 3], output: &dyn Fn(usize, f64) -> f64) -> Vec<u8> {
    let entries = if bits == 8 { 256 } else { 1024 };
    let table = |curve: &dyn Fn(usize, f64) -> f64| -> Vec<u8> {
        let mut out = vec![];
        for c in 0..3 {
            for i in 0..entries {
                let y = curve(c, i as f64 / (entries - 1) as f64).clamp(0.0, 1.0);
                if bits == 8 {
                    out.push((y * 255.0).round() as u8);
                } else {
                    out.extend(u16b((y * 65535.0).round() as i64));
                }
            }
        }
        out
    };
    let mut b: Vec<u8> = [if bits == 8 { b"mft1".to_vec() } else { b"mft2".to_vec() }, u32b(0), vec![3, 3, side as u8, 0]].concat();
    b.extend(matrix.iter().flatten().flat_map(|v| s15(*v)));
    if bits == 16 {
        b.extend(u16b(entries as i64));
        b.extend(u16b(entries as i64));
    }
    b.extend(table(input));
    b.extend(grid_values(side, bits / 8, f));
    b.extend(table(output));
    b
}

pub fn write_stand_in(path: &Path, name: &str, black: f64, paper: Option<[f64; 3]>, lab_v4: bool) {
    std::fs::write(path, stand_in(name, black, paper, lab_v4)).unwrap();
}

/// A fresh folder of the test's own, never a shared one.
pub fn sandbox(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-colour-{label}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A bluish optical-brightener paper's white, L* 94 and b* −9.
pub fn bluish() -> [f64; 3] {
    pcs::xyz_from_lab([94.0, 0.0, -9.0]).map(|v| v as f64)
}
