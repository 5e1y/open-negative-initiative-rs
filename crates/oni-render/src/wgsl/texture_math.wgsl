// Texture's per-pixel steps as pure functions, so a blur may apply one to each tap it reads.

// `textureMoments`: the first and second moment packed, so one blur carries both.
fn texture_moments_of(luma: vec4<f32>) -> vec4<f32> {
    let v = luma.r;
    return vec4<f32>(v, v * v, 0.0, 1.0);
}

// `textureCoeff`: the self-guided filter's coefficients, `a` the share of local detail the base
// follows; the threshold follows the local level, since a scene is multiplicative.
fn texture_coeff_of(m: vec4<f32>, relative: f32) -> vec4<f32> {
    let mean = m.r;
    let variance = max(m.g - mean * mean, 0.0);
    let edge = relative * max(mean, 0.0);
    let eps = max(edge * edge, 1e-8);
    let a = variance / (variance + eps);
    return vec4<f32>(a, (1.0 - a) * mean, 0.0, 1.0);
}
