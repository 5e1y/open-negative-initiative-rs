//! The De-dust correction search (`correctionSearch.metal`): the cost of each source, the bias field.

use super::*;

/// `correctionCost(src, destCentre, halfSize, windowOrigin, step, gridN, minRadius, extentMin,
/// extentMax, hole)`, every point in the source's Core Image coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CorrectionCost {
    pub dest_centre: V2,
    pub half_size: V2,
    pub window_origin: V2,
    pub step: f32,
    pub grid_n: f32,
    pub min_radius: f32,
    pub extent_min: V2,
    pub extent_max: V2,
    pub hole: V4,
}

impl CorrectionCost {
    pub fn rows(&self, source_origin: V2) -> Vec<V4> {
        vec![
            v22(self.dest_centre, self.half_size),
            [self.window_origin[0], self.window_origin[1], self.step, self.grid_n],
            v22(self.extent_min, self.extent_max),
            self.hole,
            [self.min_radius, source_origin[0], source_origin[1], 0.0],
        ]
    }
}

/// `correctionBiasField(patch, base, gradX, gradY, origin)`; `origin` in Core Image coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CorrectionBiasField {
    pub base: V3,
    pub grad_x: V3,
    pub grad_y: V3,
    pub origin: V2,
}

impl CorrectionBiasField {
    pub fn rows(&self, patch_origin: V2) -> Vec<V4> {
        vec![v3(self.base, 0.0), v3(self.grad_x, 0.0), v3(self.grad_y, 0.0), v22(self.origin, patch_origin)]
    }
}

/// Core Image's linear sample at `p` in its own frame; transparent outside the extent.
pub fn sampled(image: &Image, p: V2) -> V3 {
    let f = [p[0] - image.origin[0] - 0.5, image.height as f32 - (p[1] - image.origin[1]) - 0.5];
    let base = [f[0].floor(), f[1].floor()];
    let t = [f[0] - base[0], f[1] - base[1]];
    let (x, y) = (base[0] as i64, base[1] as i64);
    let texel = |x: i64, y: i64| {
        if x < 0 || y < 0 || x >= image.width as i64 || y >= image.height as i64 {
            [0.0; 3]
        } else {
            rgb(image.at(x as usize, y as usize))
        }
    };
    let (a, b, c, d) = (texel(x, y), texel(x + 1, y), texel(x, y + 1), texel(x + 1, y + 1));
    map([0.0; 3], |i, _| mix(mix(a[i], b[i], t[0]), mix(c[i], d[i], t[0]), t[1]))
}

/// The cost map on the extent (0, 0, `grid_size`, `grid_size`), top row first.
pub fn correction_cost(source: &Image, k: &CorrectionCost, grid_size: usize) -> Image {
    let grid = Image::new(grid_size, grid_size, vec![[0.0; 4]; grid_size * grid_size]);
    image_map(&grid, |gx, gy, _| {
        let at = grid.ci_coord(gx, gy);
        let offset = [k.window_origin[0] + at[0] * k.step, k.window_origin[1] + at[1] * k.step];
        let cand = [k.dest_centre[0] + offset[0], k.dest_centre[1] + offset[1]];
        let hs = k.half_size;
        if (offset[0] * offset[0] + offset[1] * offset[1]).sqrt() < k.min_radius
            || cand[0] - hs[0] < k.extent_min[0]
            || cand[0] + hs[0] > k.extent_max[0]
            || cand[1] - hs[1] < k.extent_min[1]
            || cand[1] + hs[1] > k.extent_max[1]
        {
            return [f32::INFINITY, offset[0], offset[1], 1.0];
        }
        let n = k.grid_n as i32;
        let hole = k.hole;
        let inside = |p: V2| p[0] >= hole[0] && p[0] <= hole[2] && p[1] >= hole[1] && p[1] <= hole[3];
        let mut accum = [0.0f32; 3];
        let mut counted = 0;
        for j in 0..n {
            for i in 0..n {
                let t = [(i as f32 + 0.5) / n as f32 * 2.0 - 1.0, (j as f32 + 0.5) / n as f32 * 2.0 - 1.0];
                let cell = [t[0] * hs[0], t[1] * hs[1]];
                let dest_pos = [k.dest_centre[0] + cell[0], k.dest_centre[1] + cell[1]];
                let src_pos = [cand[0] + cell[0], cand[1] + cell[1]];
                if inside(dest_pos) || inside(src_pos) {
                    continue;
                }
                let d = sampled(source, dest_pos);
                let s = sampled(source, src_pos);
                for c in 0..3 {
                    let diff = d[c] - s[c];
                    accum[c] += (diff * diff) / (d[c] * d[c] + 0.001);
                }
                counted += 1;
            }
        }
        if counted == 0 {
            return [f32::INFINITY, offset[0], offset[1], 1.0];
        }
        [(accum[0] + accum[1] + accum[2]) / counted as f32, offset[0], offset[1], 1.0]
    })
}

pub fn correction_bias_field(patch: &Image, k: &CorrectionBiasField) -> Image {
    image_map(patch, |x, y, p| {
        let pos = patch.ci_coord(x, y);
        let bias = map(k.base, |c, b| b + k.grad_x[c] * (pos[0] - k.origin[0]) + k.grad_y[c] * (pos[1] - k.origin[1]));
        with_alpha(map(rgb(p), |c, v| v + bias[c]), p[3])
    })
}

const CORRECTION_COST: Shader = shader!("correction_cost");

const CORRECTION_BIAS_FIELD: Shader = shader!("correction_bias_field");

impl Gpu {
    /// The cost map, (SSD, offset x, offset y, 1) per candidate on (0, 0, grid_size, grid_size); the
    /// source is read anywhere in the rings' union, so it binds whole, as the ROI asks.
    pub fn correction_cost(&self, source: &Image, k: &CorrectionCost, grid_size: usize) -> Image {
        let n = grid_size * grid_size;
        let block = uniform(head(n, grid_size, source.width, source.height), &k.rows(source.origin));
        Image::new(grid_size, grid_size, self.run(CORRECTION_COST, &block, &[&source.pixels], n, n))
    }

    /// Per pixel, the bias read at each pixel's Core Image coordinate from `patch.origin`.
    pub fn correction_bias_field(&self, patch: &Image, k: &CorrectionBiasField) -> Image {
        self.per_pixel(CORRECTION_BIAS_FIELD, |band| k.rows(band.origin), &[patch], &[], 0)
    }
}
