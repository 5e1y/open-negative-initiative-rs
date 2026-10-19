// `CIGaussianBlur` along one axis, a stage of the render graph: deviation `sigma`, each tap integrated
// over its pixel and truncated at `reach`; past the extent the edge repeats, or is transparent.
// Each tap goes through `gaussian_pre`, a per-pixel step the composing stage folds into the read.

struct Params { sigma_reach: vec4<f32>, axis_edge: vec4<u32> }

fn gaussian_read(q: vec2<i32>) -> vec4<f32> {
    if (P.axis_edge.y == 1u && !inside(q)) { return vec4<f32>(0.0); }
    return gaussian_pre(load(q));
}

fn gaussian_px(p: vec2<i32>) -> vec4<f32> {
    let step = select(vec2<i32>(1, 0), vec2<i32>(0, 1), P.axis_edge.x == 1u);
    let reach = i32(P.sigma_reach.y);
    var t = taps(P.sigma_reach.x, P.sigma_reach.y);
    var sum = tap_centre(t) * gaussian_read(p);
    for (var d = 1; d <= reach; d++) {
        sum += tap_next(&t, d) * (gaussian_read(p + d * step) + gaussian_read(p - d * step));
    }
    return sum;
}
