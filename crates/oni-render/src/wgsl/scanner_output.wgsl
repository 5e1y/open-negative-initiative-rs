// `scannerOutput`: the rendered display sRGB moved by the warmth look at its share, the look's lift
// taken back off, linearised into Rec. 2020 on the same white.

struct Params { head: vec4<u32>, share_lift: vec4<f32> }

fn scanner_output_px(s: vec4<f32>, warmth: vec4<f32>) -> vec4<f32> {
    let k = P.share_lift;
    let c = clamp(s.rgb + (warmth.rgb - k.y) * k.x, vec3<f32>(0.0), vec3<f32>(1.0));
    let linear = select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
    let out = vec3<f32>(dot(linear, vec3<f32>(0.627404, 0.329283, 0.043313)),
                        dot(linear, vec3<f32>(0.069097, 0.919540, 0.011362)),
                        dot(linear, vec3<f32>(0.016391, 0.088013, 0.895595)));
    return vec4<f32>(out, s.a);
}
