// The level of the layers' field, `load1`'s, where `x` and `y` take `q` on the scan's grid: the mean of its texels over
// a box `half` texels each side, each weighted by how much of it the box covers, or bilinearly where `half` is zero.
fn grain_plate(q: vec2<f32>, x: vec4<f32>, y: vec4<f32>, half: f32) -> vec3<f32> {
    let t = vec2<f32>(dot(x.xy, q) + x.z, dot(y.xy, q) + y.z) - 0.5;
    if (half == 0.0) {
        let f = floor(t);
        let w = t - f;
        let i = vec2<i32>(f);
        let top = mix(load1(i), load1(i + vec2<i32>(1, 0)), w.x);
        let bottom = mix(load1(i + vec2<i32>(0, 1)), load1(i + vec2<i32>(1, 1)), w.x);
        return mix(top, bottom, w.y).rgb;
    }
    let lo = t - half;
    let hi = t + half;
    let first = vec2<i32>(floor(lo + 0.5));
    var out = vec3<f32>(0.0);
    for (var n = 0; n < 5; n++) {
        let j = first.y + n;
        let wy = max(min(hi.y, f32(j) + 0.5) - max(lo.y, f32(j) - 0.5), 0.0) / (2.0 * half);
        for (var m = 0; m < 5; m++) {
            let i = first.x + m;
            let wx = max(min(hi.x, f32(i) + 0.5) - max(lo.x, f32(i) - 0.5), 0.0) / (2.0 * half);
            out += wx * wy * load1(vec2<i32>(i, j)).rgb;
        }
    }
    return out;
}

// The film at `p` read where the grains over it are centred, its layers' field off two levels laid through the gaze.
fn grain_px(p: vec2<i32>) -> vec4<f32> {
    let own = load(p);
    let c = grain_cells(vec2<f32>(p) + 0.5);
    var sample = vec3<f32>(0.0);
    var sampled = 0.0;
    for (var j = 0; j < c.count.y; j++) {
        for (var i = 0; i < c.count.x; i++) {
            var a: vec4<u32>;
            let g = grain_over(c.lo + vec2<i32>(i, j), 0u, c.q, &a);
            if (g.z <= 0.0) { continue; }
            let rp = vec2<f32>(dot(P.render_x.xyz, vec3<f32>(g.xy, 1.0)), dot(P.render_y.xyz, vec3<f32>(g.xy, 1.0)));
            sample += g.z * grain_shared(load(vec2<i32>(floor(rp))).rgb);
            sampled += g.z;
        }
    }
    let base = select(grain_shared(own.rgb), sample / sampled, sampled > 0.05);
    let field = grain_plate(c.q, P.plate[0], P.plate[1], P.blend.x) * P.blend.y;
    let delta = field * P.deviation.xyz;
    var laid = vec3<f32>(dot(P.gaze[0].xyz, delta), dot(P.gaze[1].xyz, delta), dot(P.gaze[2].xyz, delta));
    if (P.head.z == 2u) { laid = vec3<f32>(delta.x); }
    return vec4<f32>(grain_coded(base + laid), own.a);
}
