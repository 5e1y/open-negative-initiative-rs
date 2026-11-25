// The neutral negative: each layer's density a colour negative's mean tone curve of its own channel's light, coded as
// the light its scan holds less the pedestal its reading adds; alpha kept.

// k.x: one over the light the window's white stands on; k.y: the density from the bare film to it; k.z: the pedestal;
// k.w: the curve's first node in log of the light over the white; span.x: the step between two; curve: its nodes.
struct Params { head: vec4<u32>, k: vec4<f32>, span: vec4<f32>, curve: array<vec4<f32>, 16> }

fn neutral_node(i: u32) -> f32 {
    return P.curve[i / 4u][i % 4u];
}

fn neutral_tone(x: f32) -> f32 {
    let f = clamp((log2(max(x, 1e-30)) * 0.30102999566398120 - P.k.w) / P.span.x, 0.0, 63.0);
    let i = min(u32(floor(f)), 62u);
    return mix(neutral_node(i), neutral_node(i + 1u), f - f32(i));
}

fn neutral_negative_px(s: vec4<f32>) -> vec4<f32> {
    let x = s.rgb * P.k.x;
    let d = P.k.y * vec3<f32>(neutral_tone(x.r), neutral_tone(x.g), neutral_tone(x.b));
    return vec4<f32>((1.0 + P.k.z) * exp2(-3.3219280948873623 * d) - P.k.z, s.a);
}
