// A painted stroke's hard mask, a stage of the render graph: the polyline whose points the input row
// holds, thickened by `radius` with round caps and joins, white on black, its edge covered by area.

struct Params { radius_count: vec4<f32> }

fn segment_distance(c: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let l = dot(ab, ab);
    let t = select(clamp(dot(c - a, ab) / l, 0.0, 1.0), 0.0, l <= 0.0);
    return length(c - (a + t * ab));
}

fn stroke_mask_px(p: vec2<i32>) -> vec4<f32> {
    let c = vec2<f32>(p) + 0.5;
    let n = i32(P.radius_count.y);
    var d = length(c - load(vec2<i32>(0, 0)).xy);
    for (var i = 1; i < n; i++) {
        d = min(d, segment_distance(c, load(vec2<i32>(i - 1, 0)).xy, load(vec2<i32>(i, 0)).xy));
    }
    let v = clamp(P.radius_count.x + 0.5 - d, 0.0, 1.0);
    return vec4<f32>(v, v, v, 1.0);
}
