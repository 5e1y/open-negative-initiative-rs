// The pure inversion, operation for operation as `cpu::inversion`: each window share read as the display's sRGB code
// and decoded to light, the code's curve carried on past either end; alpha kept.

struct Params { head: vec4<u32> }

fn inversion_px(t: vec4<f32>) -> vec4<f32> {
    // The power's base is held above none where the straight foot is taken, so no lane computes a NaN.
    let power = pow(max((t.rgb + 0.055) / 1.055, vec3<f32>(0.0)), vec3<f32>(2.4));
    return vec4<f32>(select(power, t.rgb / 12.92, t.rgb <= vec3<f32>(0.04045)), t.a);
}
