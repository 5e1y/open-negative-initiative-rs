//! A digital photo's lens corrections by hand: radial distortion, lateral chromatic aberration and
//! optical vignetting, each a share of its slider, and the radial map they make of the scan.

use crate::cg::Point;
use serde::{Deserialize, Serialize};

/// Distortion's top as DNG WarpRectilinear's `k1`, in the scan's half-diagonal: unfolded to r = 1.29.
pub const DISTORTION_FULL: f64 = 0.2;
/// A fringe slider's top as a channel's scale against green's: about 7 px at a 24 MP corner.
pub const FRINGE_FULL: f64 = 0.002;
/// Vignetting's top in stops at the corner.
pub const VIGNETTING_FULL: f32 = 2.0;

/// The four sliders, each on −100..100, rest 0; positive corrects barrel distortion and lightens the
/// corners, and moves a fringe's channel outward, toward the colour its slider's end names.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LensCorrection {
    pub distortion: f32,
    pub red: f32,
    pub blue: f32,
    pub vignetting: f32,
}

impl LensCorrection {
    pub const RANGE: (f32, f32) = (-100.0, 100.0);

    /// The radial map on a scan of `size` pixels; `None` while distortion and both fringes rest.
    pub fn radial(&self, size: (f64, f64)) -> Option<Radial> {
        if self.distortion == 0.0 && self.red == 0.0 && self.blue == 0.0 {
            return None;
        }
        // A file may hold any number: past the range the map would fold inside the frame.
        let share = |v: f32| f64::from(v.clamp(Self::RANGE.0, Self::RANGE.1)) / 100.0;
        let k = -DISTORTION_FULL * share(self.distortion);
        // A larger channel reads nearer the centre: its slider's top shrinks its scale.
        let scale = |v: f32| 1.0 - FRINGE_FULL * share(v);
        let m = [scale(self.red), 1.0, scale(self.blue)];
        // Read through the sides' ratio alone, so a reduced copy of the same shape maps alike to the bit.
        let ratio = size.1 / size.0;
        let diagonal = libm::hypot(1.0, ratio);
        let aspect = [1.0 / diagonal, ratio / diagonal];
        let z = filling(m, k, aspect[0].min(aspect[1]));
        Some(Radial { coefficients: m.map(|m| [m * z, m * k * z * z * z, 0.0, 0.0]), aspect })
    }

    /// The optical vignetting's gain in stops at the corner, as the creative vignette's dose reads it;
    /// `None` at rest.
    pub fn vignetting_stops(&self) -> Option<f32> {
        (self.vignetting != 0.0).then(|| -VIGNETTING_FULL * self.vignetting.clamp(Self::RANGE.0, Self::RANGE.1) / 100.0)
    }
}

/// The largest zoom `z ≤ 1` whose corrected picture leaves no gap in the frame: every channel's factor
/// at most 1 at both of the boundary's extreme radii, the nearest edge's `near` and the corner's 1.
fn filling(m: [f64; 3], k: f64, near: f64) -> f64 {
    let fits = |z: f64| m.iter().all(|m| [near * near, 1.0].iter().all(|u| m * z * (1.0 + k * z * z * u) <= 1.0));
    if fits(1.0) {
        return 1.0;
    }
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..60 {
        let mid = 0.5 * (low + high);
        if fits(mid) { low = mid } else { high = mid }
    }
    low
}

/// DNG WarpRectilinear's radial form, per channel: a point at radius `r` of the corrected scan reads
/// the scan at `r·P(r²)`, `P = k0 + k1 r² + k2 r⁴ + k3 r⁶`, about the scan's centre, its half-diagonal the unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radial {
    /// Red, green, blue.
    pub coefficients: [[f64; 4]; 3],
    /// The scan's width and height over its diagonal: a fraction's offset from the centre, doubled,
    /// times these is its radius vector.
    pub aspect: [f64; 2],
}

impl Radial {
    /// Channel `c`'s factor at `u = r²`.
    pub fn factor(&self, c: usize, u: f64) -> f64 {
        let k = self.coefficients[c];
        k[0] + u * (k[1] + u * (k[2] + u * k[3]))
    }

    fn radius(&self, p: Point) -> f64 {
        libm::hypot(2.0 * (p.x - 0.5) * self.aspect[0], 2.0 * (p.y - 0.5) * self.aspect[1])
    }

    /// Where a point of the corrected scan reads the scan, both in the scan's fractions; green's.
    pub fn to_source(&self, p: Point) -> Point {
        let r = self.radius(p);
        let f = self.factor(1, r * r);
        Point::new(0.5 + f * (p.x - 0.5), 0.5 + f * (p.y - 0.5))
    }

    /// The point of the corrected scan reading `p` of the scan; one the correction pushes off the
    /// frame lands outside [0, 1].
    pub fn from_source(&self, p: Point) -> Point {
        let r = self.radius(p);
        if r == 0.0 {
            return p;
        }
        let fold = self.fold();
        // Newton on r·P(r²) = r_d, from r_d, held below the fold where the map turns back.
        let mut u = r;
        for _ in 0..12 {
            let f = u * self.factor(1, u * u) - r;
            let slope = self.slope(u * u);
            u = (u - f / slope).clamp(0.0, fold);
        }
        let f = u / r;
        Point::new(0.5 + f * (p.x - 0.5), 0.5 + f * (p.y - 0.5))
    }

    /// Green's `d(r·P(r²))/dr` at `u = r²`.
    fn slope(&self, u: f64) -> f64 {
        let k = self.coefficients[1];
        k[0] + u * (3.0 * k[1] + u * (5.0 * k[2] + u * 7.0 * k[3]))
    }

    /// The radius just short of green's first fold, where its map stops growing; far out with none.
    fn fold(&self) -> f64 {
        const FAR: f64 = 16.0;
        const WALK: u32 = 256;
        // The first sign change on a fine walk, then bisection inside it.
        let at = |i: u32| FAR * f64::from(i) / f64::from(WALK);
        let Some(i) = (1..=WALK).find(|&i| self.slope(at(i) * at(i)) <= 0.0) else {
            return FAR;
        };
        let (mut low, mut high) = (at(i - 1), at(i));
        for _ in 0..60 {
            let mid = 0.5 * (low + high);
            if self.slope(mid * mid) > 0.0 { low = mid } else { high = mid }
        }
        low * (1.0 - 1e-9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: (f64, f64) = (6000.0, 4000.0);

    fn lens(distortion: f32, red: f32, blue: f32) -> LensCorrection {
        LensCorrection { distortion, red, blue, vignetting: 0.0 }
    }

    /// The largest factor any channel reaches on the frame's edge, walked finely.
    fn edge_peak(r: &Radial) -> f64 {
        let mut peak = 0.0f64;
        for i in 0..=400 {
            let t = f64::from(i) / 400.0;
            for p in [Point::new(t, 0.0), Point::new(t, 1.0), Point::new(0.0, t), Point::new(1.0, t)] {
                let u = r.radius(p).powi(2);
                for c in 0..3 {
                    peak = peak.max(r.factor(c, u));
                }
            }
        }
        peak
    }

    #[test]
    fn the_map_rests_until_a_slider_moves_and_vignetting_is_its_own() {
        assert_eq!(LensCorrection::default().radial(SIZE), None);
        assert_eq!(LensCorrection { vignetting: 40.0, ..Default::default() }.radial(SIZE), None);
        assert_eq!(LensCorrection::default().vignetting_stops(), None);
        assert_eq!(LensCorrection { vignetting: 100.0, ..Default::default() }.vignetting_stops(), Some(-2.0));
        // Twin: a distortion alone is a map.
        assert!(lens(1.0, 0.0, 0.0).radial(SIZE).is_some());
    }

    #[test]
    fn a_pincushion_or_fringe_correction_zooms_just_enough_and_a_barrel_one_never() {
        for (d, r, b) in [(-100.0, 0.0, 0.0), (-100.0, 100.0, -100.0), (0.0, -100.0, 0.0), (-30.0, 0.0, -60.0)] {
            let radial = lens(d, r, b).radial(SIZE).unwrap();
            let peak = edge_peak(&radial);
            assert!(peak <= 1.0 + 1e-12 && peak > 1.0 - 1e-9, "{d} {r} {b}: the edge's peak factor {peak}");
        }
        // A barrel correction keeps the frame's own scale.
        let barrel = lens(100.0, 0.0, 0.0).radial(SIZE).unwrap();
        assert_eq!(barrel.coefficients[1][0], 1.0);
        // Twin: unzoomed, the pincushion correction reads past the frame.
        let zoomed = lens(-100.0, 0.0, 0.0).radial(SIZE).unwrap();
        let unzoomed = Radial { coefficients: [[1.0, DISTORTION_FULL, 0.0, 0.0]; 3], ..zoomed };
        assert!(zoomed.coefficients[1][0] < 1.0 && edge_peak(&unzoomed) > 1.1);
    }

    #[test]
    fn the_zoom_reads_the_aspect_alone_never_the_scale() {
        // A slight barrel correction under a red fringe's: the edge's middle binds, so the aspect counts.
        let fringed = lens(2.0, -100.0, 0.0);
        let full = fringed.radial(SIZE).unwrap();
        assert_eq!(full, fringed.radial((SIZE.0 * 0.25, SIZE.1 * 0.25)).unwrap());
        assert!(full.coefficients[1][0] < 1.0);
        // Twin: a square scan zooms otherwise.
        assert_ne!(full.coefficients, fringed.radial((SIZE.0, SIZE.0)).unwrap().coefficients);
    }

    #[test]
    fn from_source_undoes_to_source_across_the_frame() {
        for d in [-100.0, -40.0, 35.0, 100.0] {
            let radial = lens(d, 50.0, -50.0).radial(SIZE).unwrap();
            for i in 0..=20 {
                for j in 0..=20 {
                    let p = Point::new(f64::from(i) / 20.0, f64::from(j) / 20.0);
                    let back = radial.from_source(radial.to_source(p));
                    assert!((back.x - p.x).abs() < 1e-9 && (back.y - p.y).abs() < 1e-9, "{d}: {p:?} back at {back:?}");
                }
            }
        }
        // A pincushion correction's zoom keeps the scan's corner on the frame's and pushes its edges off.
        let zoomed = lens(-100.0, 0.0, 0.0).radial(SIZE).unwrap();
        let corner = zoomed.from_source(Point::new(1.0, 1.0));
        assert!((corner.x - 1.0).abs() < 1e-9 && (corner.y - 1.0).abs() < 1e-9, "{corner:?}");
        let pushed = zoomed.from_source(Point::new(1.0, 0.5));
        assert!(pushed.x > 1.0 && pushed.y == 0.5, "{pushed:?}");
        // Twin: unzoomed, the same correction keeps the edge on the frame.
        let unzoomed = Radial { coefficients: [[1.0, DISTORTION_FULL, 0.0, 0.0]; 3], ..zoomed };
        assert!(unzoomed.from_source(Point::new(1.0, 0.5)).x < 1.0);
    }

    #[test]
    fn a_value_past_the_range_maps_as_its_end() {
        assert_eq!(lens(1000.0, -500.0, 300.0).radial(SIZE), lens(100.0, -100.0, 100.0).radial(SIZE));
        assert_eq!(LensCorrection { vignetting: 900.0, ..Default::default() }.vignetting_stops(), Some(-2.0));
        // Twin: inside the range a value is its own.
        assert_ne!(lens(99.0, 0.0, 0.0).radial(SIZE), lens(100.0, 0.0, 0.0).radial(SIZE));
    }

    #[test]
    fn the_map_never_folds_inside_the_frame() {
        let barrel = lens(100.0, 0.0, 0.0).radial(SIZE).unwrap();
        assert!((barrel.fold() - (1.0 / (3.0 * DISTORTION_FULL)).sqrt()).abs() < 1e-6, "{}", barrel.fold());
        assert!(barrel.fold() > 1.29);
        // Twin: a pincushion correction never folds.
        assert_eq!(lens(-100.0, 0.0, 0.0).radial(SIZE).unwrap().fold(), 16.0);
    }
}
