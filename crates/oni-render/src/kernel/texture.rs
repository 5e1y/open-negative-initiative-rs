//! Texture (`chroma.metal`): the local moments, their regression, the gain on what departs from it.

use super::*;

/// `textureCoeff(moments, relative)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureCoeff {
    pub relative: f32,
}

/// `texture(src, narrowLuma, coeff, gain)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Texture {
    pub gain: f32,
}

pub fn texture_moments(luma: V4) -> V4 {
    [luma[0], luma[0] * luma[0], 0.0, 1.0]
}

pub fn texture_coeff(m: V4, k: &TextureCoeff) -> V4 {
    let mean = m[0];
    let variance = (m[1] - mean * mean).max(0.0);
    let edge = k.relative * mean.max(0.0);
    let eps = (edge * edge).max(1e-8);
    let a = variance / (variance + eps);
    [a, (1.0 - a) * mean, 0.0, 1.0]
}

pub fn texture(s: V4, narrow_luma: V4, coeff: V4, k: &Texture) -> V4 {
    let y = dot(rgb(s), LUMA);
    let n = narrow_luma[0];
    let base = coeff[0] * n + coeff[1];
    let shaped = y + (n - base) * k.gain;
    with_alpha(map(rgb(s), |_, c| shaped.max(0.0) + (c - y)), s[3])
}

const TEXTURE_MOMENTS: Shader = shader!("texture_moments", crate::wgsl::TEXTURE_MATH);

const TEXTURE_COEFF: Shader = shader!("texture_coeff", crate::wgsl::TEXTURE_MATH);

const TEXTURE: Shader = shader!("texture");

impl Gpu {
    pub fn texture_moments(&self, luma: &Image) -> Image {
        self.per_pixel(TEXTURE_MOMENTS, |_| Vec::new(), &[luma], &[], 0)
    }

    pub fn texture_coeff(&self, moments: &Image, k: &TextureCoeff) -> Image {
        self.per_pixel(TEXTURE_COEFF, |_| vec![[k.relative, 0.0, 0.0, 0.0]], &[moments], &[], 0)
    }

    pub fn texture(&self, image: &Image, narrow_luma: &Image, coeff: &Image, k: &Texture) -> Image {
        self.per_pixel(TEXTURE, |_| vec![[k.gain, 0.0, 0.0, 0.0]], &[image, narrow_luma, coeff], &[], 0)
    }
}
