// `outputClip`: the hard, constant clip, the only bound above in the chain.

struct Params { head: vec4<u32> }

fn output_clip_px(s: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(clamp(s.rgb, vec3<f32>(0.0), vec3<f32>(1.0)), s.a);
}
