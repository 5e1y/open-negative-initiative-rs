// `flatField`: the linear negative divided by its mask, mixed toward 1 by `amount`, then raised per channel.

struct Params {
    head: vec4<u32>,
    amount_per_channel: vec4<f32>,
    exponent: vec4<f32>,
}

fn flat_field_px(s: vec4<f32>, f: vec4<f32>) -> vec4<f32> {
    let amount = P.amount_per_channel.x;
    let m = select(vec3<f32>(max(f.g, 0.05)), max(f.rgb, vec3<f32>(0.05)), P.amount_per_channel.y > 0.5);
    let base = mix(vec3<f32>(1.0), m, amount);
    // pow(1, y) is exactly 1 in C and Metal, not in every WGSL implementation: amount 0 is the identity.
    let d = select(pow(base, P.exponent.xyz), vec3<f32>(1.0), base == vec3<f32>(1.0));
    return vec4<f32>(s.rgb / d, s.a);
}
