// An affine resampling, a stage of the render graph: each output pixel's centre mapped back onto the
// input and read bilinearly, past the extent transparent as Core Image's transform, or the edge repeated.

struct Params { row_x: vec4<f32>, row_y: vec4<f32>, edge: vec4<u32> }

fn affine_read(q: vec2<i32>) -> vec4<f32> {
    if (P.edge.x == 1u && !inside(q)) { return vec4<f32>(0.0); }
    return load(q);
}

fn affine_px(p: vec2<i32>) -> vec4<f32> {
    let c = vec2<f32>(p) + 0.5;
    let at = vec2<f32>(dot(P.row_x.xy, c) + P.row_x.z, dot(P.row_y.xy, c) + P.row_y.z) - 0.5;
    let f = floor(at);
    let t = at - f;
    let q = vec2<i32>(f);
    let top = mix(affine_read(q), affine_read(q + vec2<i32>(1, 0)), t.x);
    let bottom = mix(affine_read(q + vec2<i32>(0, 1)), affine_read(q + vec2<i32>(1, 1)), t.x);
    return mix(top, bottom, t.y);
}
