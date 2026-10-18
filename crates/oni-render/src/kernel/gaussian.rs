//! The render graph's blur, `CIGaussianBlur` along one axis.

use super::*;

/// `CIGaussianBlur` of radius `sigma` along one axis; `transparent` reads zero past the extent
/// instead of its edge, as a blur of an image that was not `clampedToExtent`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gaussian {
    pub sigma: f32,
    pub vertical: bool,
    pub transparent: bool,
}

impl Gaussian {
    /// The taps each side, out to 3.5 deviations as the detail stage reads them.
    pub fn reach(&self) -> i32 {
        detail_reach(f64::from(self.sigma))
    }

    pub fn bytes(&self) -> Vec<u8> {
        let head = [self.sigma, self.reach() as f32, 0.0, 0.0];
        let flags = [u32::from(self.vertical), u32::from(self.transparent), 0, 0];
        [bytemuck::bytes_of(&head), bytemuck::bytes_of(&flags)].concat()
    }
}

/// `CIGaussianBlur` along one axis, on the input's own extent.
pub fn gaussian(input: &Grid, k: &crate::Gaussian) -> Grid {
    let reach = k.reach();
    let step = if k.vertical { [0, 1] } else { [1, 0] };
    Grid::generated(input.x, input.y, input.width, input.height, |p| {
        let mut taps = Taps::new(k.sigma, reach as f32);
        let centre = taps.centre();
        let mut sum = input.read(p, k.transparent).map(|v| v * centre);
        for d in 1..=reach {
            let w = taps.next(d);
            let a = input.read([p[0] + d * step[0], p[1] + d * step[1]], k.transparent);
            let b = input.read([p[0] - d * step[0], p[1] - d * step[1]], k.transparent);
            for c in 0..4 {
                sum[c] += w * (a[c] + b[c]);
            }
        }
        sum
    })
}
