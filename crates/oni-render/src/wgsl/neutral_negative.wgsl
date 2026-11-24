// The neutral negative: each layer's density the screen's code of its own channel's light, coded as the light its
// scan holds less the pedestal its reading adds; alpha kept.

// k.x: one over the light the code's 1 stands on; k.y: the density from the bare film to that 1; k.z: the pedestal.
struct Params { head: vec4<u32>, k: vec4<f32> }

fn neutral_negative_px(s: vec4<f32>) -> vec4<f32> {
    let x = s.rgb * P.k.x;
    let code = select(1.055 * pow(max(x, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055, 12.92 * x, x <= vec3<f32>(0.0031308));
    return vec4<f32>((1.0 + P.k.z) * exp2(-3.3219280948873623 * P.k.y * code) - P.k.z, s.a);
}
