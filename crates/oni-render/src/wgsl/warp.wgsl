// The lens warp, a stage of the render graph: each output pixel's centre mapped through stage 0's
// turns and angle onto the corrected scan, then each channel read bilinearly where its radial map
// lands it on the scan; past the extent transparent, or the edge repeated.

struct Params {
    head: vec4<u32>,
    row_x: vec4<f32>,
    row_y: vec4<f32>,
    centre: vec4<f32>,
    red: vec4<f32>,
    green: vec4<f32>,
    blue: vec4<f32>,
}

fn warp_read(q: vec2<i32>) -> vec4<f32> {
    if (P.head.x == 1u && !inside(q)) { return vec4<f32>(0.0); }
    return load(q);
}

fn warp_bilinear(at: vec2<f32>) -> vec4<f32> {
    let a = at - 0.5;
    let f = floor(a);
    let t = a - f;
    let q = vec2<i32>(f);
    let top = mix(warp_read(q), warp_read(q + vec2<i32>(1, 0)), t.x);
    let bottom = mix(warp_read(q + vec2<i32>(0, 1)), warp_read(q + vec2<i32>(1, 1)), t.x);
    return mix(top, bottom, t.y);
}

fn warp_factor(k: vec4<f32>, u: f32) -> f32 {
    return k.x + u * (k.y + u * (k.z + u * k.w));
}

fn warp_px(p: vec2<i32>) -> vec4<f32> {
    let c = vec2<f32>(p) + 0.5;
    let s = vec2<f32>(P.row_x.x * c.x + P.row_x.y * c.y + P.row_x.z, P.row_y.x * c.x + P.row_y.y * c.y + P.row_y.z);
    let d = s - P.centre.xy;
    let u = (d.x * d.x + d.y * d.y) * P.centre.z;
    let g = warp_bilinear(P.centre.xy + warp_factor(P.green, u) * d);
    // Red and blue resting on green's map: one fetch.
    if (P.head.y == 0u) { return g; }
    let r = warp_bilinear(P.centre.xy + warp_factor(P.red, u) * d);
    let b = warp_bilinear(P.centre.xy + warp_factor(P.blue, u) * d);
    return vec4<f32>(r.r, g.g, b.b, g.a);
}
