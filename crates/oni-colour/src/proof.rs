//! The canvas drawn as the export will write it: the proof every picture the editor shows goes
//! through, before the display profile. Never optional.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use moxcms::ColorProfile;

use crate::gamut::Display;
use crate::legs::{self, Leg};
use crate::paper::PaperCorrection;
use crate::pcs;
use crate::profile::OutputProfile;
use crate::space::Space;
use crate::working::{rec2020_decode, rec2020_encode};

#[derive(Debug, Clone, PartialEq)]
pub struct Proof {
    pub space: Space,
    /// The paper's white as white and its black as the screen's. Off at rest: the canvas shows the print.
    pub colour_only: bool,
    /// The photograph's paper gains, laid before the profile on a paper only.
    pub correction: PaperCorrection,
}

/// A paper profile baked into a cube on gamma-encoded Rec. 2020, RGBA, red varying fastest.
#[derive(Debug, Clone, PartialEq)]
pub struct Cube {
    pub side: usize,
    pub data: Vec<f32>,
}

/// Locked: the canvas asks on its own thread, thumbnails off it.
static CUBES: LazyLock<Mutex<HashMap<String, (crate::profile::Stamp, Arc<Cube>)>>> = LazyLock::new(Default::default);

impl Proof {
    pub const CUBE_SIDE: usize = 33;

    pub fn new(space: Space) -> Self {
        Proof { space, colour_only: false, correction: PaperCorrection::default() }
    }

    pub fn correcting(&self, correction: PaperCorrection) -> Proof {
        Proof { correction, ..self.clone() }
    }

    /// Working-space RGBA after the export's own conversion, back in the working space; `None`
    /// when the profile cannot be read.
    pub fn proofed(&self, pixels: &[f32]) -> Option<Vec<f32>> {
        let mut out = pixels.to_vec();
        if let Space::Profile(profile) = &self.space {
            let cube = Self::cube(profile, self.colour_only)?;
            if self.space.paper().is_some() {
                self.correction.apply(&mut out);
            }
            cube.apply(&mut out);
            return Some(out);
        }
        let m = self.space.from_working()?;
        let back = pcs::inverse(&m)?;
        for p in out.chunks_exact_mut(4) {
            let linear = pcs::mul_f32(&m, [p[0], p[1], p[2]]).map(|v| v.clamp(0.0, 1.0));
            p[..3].copy_from_slice(&pcs::mul_f32(&back, linear));
        }
        Some(out)
    }

    /// The proof, or the picture as it is when the profile cannot be read rather than a blank.
    pub fn applied(&self, pixels: &[f32]) -> Vec<f32> {
        self.proofed(pixels).unwrap_or_else(|| pixels.to_vec())
    }

    /// True when a primary of the space lands outside the display's gamut. Reads the two
    /// profiles, never the picture.
    pub fn shows_less(display: &ColorProfile, than: &Space) -> bool {
        // Borrowed, not cloned: the bar asks on every pass and a paper's tables run to 10 MB.
        let loaded = match than {
            Space::Profile(p) => p.loaded(),
            _ => None,
        };
        let source = match (&loaded, than.written()) {
            (Some(l), _) => &l.profile,
            (None, Some(w)) => &w.1,
            (None, None) => return false,
        };
        let Some(display) = Display::new(display) else { return false };
        let primaries = [[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0]];
        let xyz: Vec<[f32; 3]> = if source.is_matrix_shaper() && source.lut_a_to_b_colorimetric.is_none() {
            let m = source.rgb_to_xyz_matrix().v;
            primaries.iter().map(|p| pcs::mul_f32(&m, [p[0], p[1], p[2]])).collect()
        } else {
            let Some(out) = Leg::to_xyz(source).and_then(|l| l.xyz(primaries.as_flattened())) else {
                return false;
            };
            out.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect()
        };
        xyz.iter().any(|x| display.beyond_xyz(*x))
    }

    // MARK: - A paper profile as a cube

    pub fn cube(profile: &OutputProfile, colour_only: bool) -> Option<Arc<Cube>> {
        let key = format!("{}|{}", profile.path, colour_only);
        let stamp = crate::profile::stamp(std::path::Path::new(&profile.path))?;
        if let Some((held, cube)) = CUBES.lock().unwrap().get(&key)
            && *held == stamp
        {
            return Some(cube.clone());
        }
        let built = Arc::new(Self::build_cube(profile, colour_only)?);
        CUBES.lock().unwrap().insert(key, (stamp, built.clone()));
        Some(built)
    }

    /// Working space to the profile and back: the export's own legs, then the paper read back.
    pub fn build_cube(profile: &OutputProfile, colour_only: bool) -> Option<Cube> {
        let data = std::fs::read(&profile.path).ok()?;
        let legs = legs::legs(&data)?;
        let back = legs::legs_back(&data)?;
        let n = Self::CUBE_SIDE;
        let step = 1.0 / (n - 1) as f32;
        let mut grid = Vec::with_capacity(n * n * n * 4);
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    grid.extend([r, g, b].map(|i| rec2020_decode(i as f32 * step)));
                    grid.push(1.0);
                }
            }
        }
        let device = legs::run(&legs, &grid)?;
        let mut cube = legs::run_back(&back, &device, colour_only.then_some(&legs))?;
        for p in cube.chunks_exact_mut(4) {
            for v in &mut p[..3] {
                *v = rec2020_encode(*v).clamp(0.0, 1.0);
            }
            p[3] = 1.0;
        }
        Some(Cube { side: n, data: cube })
    }
}

impl Cube {
    fn at(&self, r: usize, g: usize, b: usize) -> &[f32] {
        let i = ((b * self.side + g) * self.side + r) * 4;
        &self.data[i..i + 3]
    }

    /// The cube on working-space RGBA: encoded, looked up trilinearly, decoded; alpha kept.
    pub fn apply(&self, pixels: &mut [f32]) {
        let last = (self.side - 1) as f32;
        for p in pixels.chunks_exact_mut(4) {
            let at = [p[0], p[1], p[2]].map(|v| rec2020_encode(v).clamp(0.0, 1.0) * last);
            let lo = at.map(|v| (v.floor() as usize).min(self.side - 2));
            let t: [f32; 3] = std::array::from_fn(|i| at[i] - lo[i] as f32);
            let mut out = [0.0f32; 3];
            for corner in 0..8 {
                let pick = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
                let weight: f32 = (0..3).map(|i| if pick[i] == 1 { t[i] } else { 1.0 - t[i] }).product();
                if weight == 0.0 {
                    continue;
                }
                let v = self.at(lo[0] + pick[0], lo[1] + pick[1], lo[2] + pick[2]);
                for c in 0..3 {
                    out[c] += weight * v[c];
                }
            }
            for c in 0..3 {
                p[c] = rec2020_decode(out[c]);
            }
        }
    }
}
