//! A profile's A2B or B2A table evaluated stage by stage, each pixel interpolated once: a grid
//! baked from the whole transform interpolates twice and moves device values by up to 0.07.

use moxcms::{DataColorSpace, LutStore, LutType, LutWarehouse, ToneReprCurve};

use crate::curve::{Curve, sampled};
use crate::pcs;

/// How the PCS side of a table encodes its values on [0, 1].
#[derive(Debug, Clone, Copy, PartialEq)]
enum Coding {
    /// u1Fixed15: 1.0 is 0x8000 of 0xFFFF.
    Xyz,
    /// `lut16Type`'s Lab, where L* 100 is 0xFF00.
    LabLegacy,
    LabV4,
}

struct Clut {
    grid: [usize; 3],
    values: Vec<f32>,
}

enum Stages {
    /// `lut8Type` and `lut16Type`: matrix on XYZ input, curves, grid, curves.
    Lut { matrix: Option<pcs::Mat3>, input: [Vec<f64>; 3], clut: Clut, output: [Vec<f64>; 3] },
    /// `lutAtoBType` runs A, grid, M, matrix, B; `lutBtoAType` runs them in the reverse order. An
    /// absent matrix reads as the identity.
    Multi { a: Option<Box<[Curve; 3]>>, clut: Option<Clut>, m: Option<Box<[Curve; 3]>>, matrix: (pcs::Mat3, [f64; 3]), b: Option<Box<[Curve; 3]>> },
}

pub struct Table {
    stages: Stages,
    coding: Coding,
    to_pcs: bool,
    /// A grid indexed by Lab, a B2A on a Lab PCS, interpolates trilinearly as lcms2 does; every
    /// other grid tetrahedrally.
    trilinear: bool,
}

fn normalised(store: &LutStore) -> Vec<f32> {
    match store {
        LutStore::Store8(v) => v.iter().map(|&x| x as f32 / 255.0).collect(),
        LutStore::Store16(v) => v.iter().map(|&x| x as f32 / 65535.0).collect(),
    }
}

fn curves(list: &[ToneReprCurve]) -> Option<Option<Box<[Curve; 3]>>> {
    if list.is_empty() {
        return Some(None);
    }
    if list.len() != 3 {
        return None;
    }
    let made: Vec<Curve> = list.iter().map(Curve::new).collect::<Option<_>>()?;
    let mut it = made.into_iter();
    Some(Some(Box::new([it.next()?, it.next()?, it.next()?])))
}

impl Clut {
    fn node(&self, i: [usize; 3]) -> [f32; 3] {
        let k = ((i[0] * self.grid[1] + i[1]) * self.grid[2] + i[2]) * 3;
        [self.values[k], self.values[k + 1], self.values[k + 2]]
    }

    fn eval(&self, v: [f32; 3], trilinear: bool) -> [f32; 3] {
        let mut lo = [0usize; 3];
        let mut t = [0.0f32; 3];
        for d in 0..3 {
            let at = v[d].clamp(0.0, 1.0) * (self.grid[d] - 1) as f32;
            lo[d] = (at.floor() as usize).min(self.grid[d] - 2);
            t[d] = at - lo[d] as f32;
        }
        let corner = |a: usize, b: usize, c: usize| self.node([lo[0] + a, lo[1] + b, lo[2] + c]);
        let mix = |acc: &mut [f32; 3], w: f32, n: [f32; 3]| (0..3).for_each(|c| acc[c] += w * n[c]);
        let mut out = [0.0f32; 3];
        if trilinear {
            for k in 0..8 {
                let p = [k >> 2 & 1, k >> 1 & 1, k & 1];
                let w: f32 = (0..3).map(|d| if p[d] == 1 { t[d] } else { 1.0 - t[d] }).product();
                mix(&mut out, w, corner(p[0], p[1], p[2]));
            }
            return out;
        }
        // The cube split along its diagonal into six tetrahedra, as lcms2 interpolates.
        let (rx, ry, rz) = (t[0], t[1], t[2]);
        let c0 = corner(0, 0, 0);
        let c111 = corner(1, 1, 1);
        let (w, a, b) = if rx >= ry && ry >= rz {
            ([1.0 - rx, rx - ry, ry - rz, rz], corner(1, 0, 0), corner(1, 1, 0))
        } else if rx >= rz && rz >= ry {
            ([1.0 - rx, rx - rz, rz - ry, ry], corner(1, 0, 0), corner(1, 0, 1))
        } else if rz >= rx && rx >= ry {
            ([1.0 - rz, rz - rx, rx - ry, ry], corner(0, 0, 1), corner(1, 0, 1))
        } else if ry >= rx && rx >= rz {
            ([1.0 - ry, ry - rx, rx - rz, rz], corner(0, 1, 0), corner(1, 1, 0))
        } else if ry >= rz && rz >= rx {
            ([1.0 - ry, ry - rz, rz - rx, rx], corner(0, 1, 0), corner(0, 1, 1))
        } else {
            ([1.0 - rz, rz - ry, ry - rx, rx], corner(0, 0, 1), corner(0, 1, 1))
        };
        mix(&mut out, w[0], c0);
        mix(&mut out, w[1], a);
        mix(&mut out, w[2], b);
        mix(&mut out, w[3], c111);
        out
    }
}

fn apply(curves: &Option<Box<[Curve; 3]>>, v: [f32; 3]) -> [f32; 3] {
    match curves {
        Some(c) => std::array::from_fn(|i| c[i].eval(v[i].clamp(0.0, 1.0) as f64) as f32),
        None => v,
    }
}

impl Table {
    /// `None` for a table that is not three channels in and out.
    pub fn new(table: &LutWarehouse, pcs: DataColorSpace, to_pcs: bool) -> Option<Table> {
        let lab = pcs == DataColorSpace::Lab;
        let (stages, coding) = match table {
            LutWarehouse::Lut(lut) => {
                if lut.num_input_channels != 3 || lut.num_output_channels != 3 {
                    return None;
                }
                let split = |store: &LutStore, entries: usize| -> Option<[Vec<f64>; 3]> {
                    let all: Vec<f64> = normalised(store).into_iter().map(f64::from).collect();
                    (all.len() >= entries * 3).then(|| std::array::from_fn(|i| all[i * entries..(i + 1) * entries].to_vec()))
                };
                let g = lut.num_clut_grid_points as usize;
                let values = normalised(&lut.clut_table);
                if g < 2 || values.len() != g * g * g * 3 {
                    return None;
                }
                let identity = lut.matrix.v == [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
                let stages = Stages::Lut {
                    matrix: (!to_pcs && !lab && !identity).then_some(lut.matrix.v),
                    input: split(&lut.input_table, lut.num_input_table_entries as usize)?,
                    clut: Clut { grid: [g; 3], values },
                    output: split(&lut.output_table, lut.num_output_table_entries as usize)?,
                };
                let coding = match (lab, lut.lut_type) {
                    (false, _) => Coding::Xyz,
                    (true, LutType::Lut16) => Coding::LabLegacy,
                    (true, _) => Coding::LabV4,
                };
                (stages, coding)
            }
            LutWarehouse::Multidimensional(m) => {
                if m.num_input_channels != 3 || m.num_output_channels != 3 {
                    return None;
                }
                let clut = match &m.clut {
                    Some(store) => {
                        let grid = [m.grid_points[0] as usize, m.grid_points[1] as usize, m.grid_points[2] as usize];
                        let values = normalised(store);
                        if grid.iter().any(|&g| g < 2) || values.len() != grid.iter().product::<usize>() * 3 {
                            return None;
                        }
                        Some(Clut { grid, values })
                    }
                    None => None,
                };
                let stages = Stages::Multi {
                    a: curves(&m.a_curves)?,
                    clut,
                    m: curves(&m.m_curves)?,
                    matrix: (m.matrix.v, [m.bias.v[0], m.bias.v[1], m.bias.v[2]]),
                    b: curves(&m.b_curves)?,
                };
                (stages, if lab { Coding::LabV4 } else { Coding::Xyz })
            }
        };
        Some(Table { stages, coding, to_pcs, trilinear: lab && !to_pcs })
    }

    /// The table on normalised values, its own direction.
    fn eval(&self, v: [f32; 3]) -> [f32; 3] {
        match &self.stages {
            Stages::Lut { matrix, input, clut, output } => {
                let v = match matrix {
                    Some(m) => pcs::mul_f32(m, v),
                    None => v,
                };
                let v: [f32; 3] = std::array::from_fn(|i| sampled(&input[i], v[i] as f64) as f32);
                let v = clut.eval(v, self.trilinear);
                std::array::from_fn(|i| sampled(&output[i], v[i] as f64) as f32)
            }
            Stages::Multi { a, clut, m, matrix, b } => {
                let grid = |v: [f32; 3]| match clut {
                    Some(c) => c.eval(v, self.trilinear),
                    None => v,
                };
                let affine = |v: [f32; 3]| -> [f32; 3] {
                    let o = pcs::mul(&matrix.0, v.map(|x| x as f64));
                    std::array::from_fn(|i| (o[i] + matrix.1[i]) as f32)
                };
                if self.to_pcs {
                    apply(b, affine(apply(m, grid(apply(a, v)))))
                } else {
                    apply(a, grid(apply(m, affine(apply(b, v)))))
                }
            }
        }
    }

    /// Device values to D50 XYZ, for an A2B table.
    pub fn xyz(&self, device: [f32; 3]) -> [f32; 3] {
        let e = self.eval(device);
        match self.coding {
            Coding::Xyz => e.map(|v| v * 65535.0 / 32768.0),
            Coding::LabLegacy => {
                let k = 65535.0 / 65280.0;
                pcs::xyz_from_lab([e[0] * k * 100.0, e[1] * k * 255.0 - 128.0, e[2] * k * 255.0 - 128.0])
            }
            Coding::LabV4 => pcs::xyz_from_lab([e[0] * 100.0, e[1] * 255.0 - 128.0, e[2] * 255.0 - 128.0]),
        }
    }

    /// D50 XYZ to device values, for a B2A table.
    pub fn device(&self, xyz: [f32; 3]) -> [f32; 3] {
        let e = match self.coding {
            Coding::Xyz => xyz.map(|v| v * 32768.0 / 65535.0),
            Coding::LabLegacy | Coding::LabV4 => {
                let l = pcs::lab(xyz);
                let v4 = [l[0] / 100.0, (l[1] + 128.0) / 255.0, (l[2] + 128.0) / 255.0];
                if self.coding == Coding::LabLegacy { v4.map(|v| v * 65280.0 / 65535.0) } else { v4 }
            }
        };
        self.eval(e)
    }
}
