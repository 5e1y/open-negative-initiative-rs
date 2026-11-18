//! The contrast slider's transfer function: a normalised logistic and its exact inverse.

pub const FULL_STEEPNESS: f32 = 15.2;
const DEAD: f32 = 1e-4;

pub fn apply(x: f32, travel: f32, pivot: f32) -> f32 {
    if pivot == 0.5 || (travel.abs() * FULL_STEEPNESS) <= DEAD {
        return symmetric(x, travel);
    }
    let warp = libm::logf(0.5) / libm::logf(pivot);
    libm::powf(symmetric(libm::powf(x.clamp(0.0, 1.0), warp), travel), 1.0 / warp)
}

fn symmetric(x: f32, travel: f32) -> f32 {
    let b = travel.abs() * FULL_STEEPNESS;
    if b <= DEAD {
        return x;
    }
    let low = logistic(0.0, b);
    let high = logistic(1.0, b);
    if travel > 0.0 {
        return (logistic(x, b) - low) / (high - low);
    }
    let g = low + x.clamp(0.0, 1.0) * (high - low);
    (0.5 - libm::logf(1.0 / g - 1.0) / b).clamp(0.0, 1.0)
}

fn logistic(x: f32, b: f32) -> f32 {
    1.0 / (1.0 + libm::expf(-b * (x - 0.5)))
}
