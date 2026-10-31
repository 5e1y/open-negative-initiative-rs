// A positive read as its own negative: each channel's light inverted about `k.x`, never under `k.y`.

struct Params { head: vec4<u32>, k: vec4<f32> }

fn reciprocal_px(s: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(P.k.x / max(s.rgb, vec3<f32>(P.k.y)), s.a);
}
