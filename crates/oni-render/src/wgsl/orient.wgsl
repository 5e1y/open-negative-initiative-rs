// Quarter turns and mirrors, a stage of the render graph: an exact permutation of pixels, the input
// pixel at `a * p + b` for output pixel `p`, with no resampling.

struct Params { a: vec4<i32>, b: vec4<i32> }

fn orient_px(p: vec2<i32>) -> vec4<f32> {
    return load(vec2<i32>(P.a.x * p.x + P.a.y * p.y + P.b.x, P.a.z * p.x + P.a.w * p.y + P.b.y));
}
