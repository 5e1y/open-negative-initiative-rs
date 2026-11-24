// The pure inversion, operation for operation as `cpu::inversion`: each window share, held within 0 and 1, read as the
// display's sRGB code and decoded to light; alpha kept.

struct Params { head: vec4<u32> }

fn inversion_px(t: vec4<f32>) -> vec4<f32> {
    let code = clamp(t.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let light = select(pow((code + 0.055) / 1.055, vec3<f32>(2.4)), code / 12.92, code <= vec3<f32>(0.04045));
    return vec4<f32>(light, t.a);
}
