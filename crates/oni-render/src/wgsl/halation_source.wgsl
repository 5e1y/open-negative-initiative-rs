// `halation_source`: the light a pixel sends into its glow, its exposure weighted by the square of its share of the
// sensor's saturation; alpha zero.

struct Params { head: vec4<u32>, dose: vec4<f32>, saturation: vec4<f32> }

fn halation_source_px(e: vec4<f32>) -> vec4<f32> {
    let w = clamp(e.rgb / P.saturation.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(e.rgb * w * w, 0.0);
}
