// A slide's black held under Lighting: the lit picture laid back onto its rest over the deep shadows, none of the move
// where the rest shows the film's black, the whole from a reach of light above it, a smoothstep between; alpha the lit's.

struct Params {
    head: vec4<u32>,
    black_reach: vec4<f32>,
}

fn held_black_px(lit: vec4<f32>, rest: vec4<f32>) -> vec4<f32> {
    let t = clamp((dot(rest.rgb, vec3<f32>(0.2627, 0.6780, 0.0593)) - P.black_reach.x) / P.black_reach.y, 0.0, 1.0);
    let share = t * t * (3.0 - 2.0 * t);
    return vec4<f32>(rest.rgb + share * (lit.rgb - rest.rgb), lit.a);
}
