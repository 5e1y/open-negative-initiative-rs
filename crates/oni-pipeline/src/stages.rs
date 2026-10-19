//! Every kernel as a render-graph stage: oni-render's body behind a `fn pixel`, the region it reads
//! and the extent it writes. The parameters are the bytes the kernel's own `Params` lays out.

use oni_graph::{Rect, Stage};
use oni_render::{
    Affine, ColorMatrix, Curves, Detail, FlatField, Gaussian, LUT_SIZE, Orient, SaturationCurve, ScannerDensity,
    Spectrogram, Stage47, StrokeMask, TextureCoeff, Vignette, uniform, wgsl,
};
use std::sync::LazyLock;

/// The shared prelude, then each part's body, the uniform at the graph's binding 3, then `wrapper`.
fn compose(parts: &[&str], wrapper: &str) -> String {
    let mut out = String::from(wgsl::PRELUDE);
    for part in parts {
        out.push('\n');
        out.push_str(wgsl::body(part));
    }
    out.push_str("\n@group(0) @binding(3) var<uniform> P: Params;\n");
    out.push_str(wrapper);
    out
}

macro_rules! shader {
    ($name:ident, [$($part:expr),*], $wrapper:expr) => {
        static $name: LazyLock<String> = LazyLock::new(|| compose(&[$($part),*], $wrapper));
    };
}

macro_rules! kernel {
    ($name:ident, $part:expr, call $call:literal) => {
        static $name: Kernel = Kernel {
            shader: LazyLock::new(|| compose(&[$part], concat!("fn pixel(p: vec2<i32>) -> vec4<f32> { let v = load(p); return ", $call, "; }"))),
            alone: Some(($part, $call)),
        };
    };
    ($name:ident, [$($part:expr),*], $wrapper:expr) => {
        static $name: Kernel = Kernel { shader: LazyLock::new(|| compose(&[$($part),*], $wrapper)), alone: None };
    };
}

const HEAD: [u32; 4] = [0; 4];

mod per_pixel;
mod fused;
mod tabled;
mod detail;
mod resampling;
mod stroke;
mod plain;
mod patch;

pub use per_pixel::*;
pub use fused::{Fused, laid};
pub use tabled::*;
pub use detail::*;
pub use resampling::*;
pub use stroke::*;
pub use plain::*;
pub use patch::*;
