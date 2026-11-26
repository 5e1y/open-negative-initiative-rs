// The film at `p` read where the grains over it are centred, its layers' field `load1(p)` laid through the gaze.
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
    let delta = load1(p).rgb * P.deviation.xyz;
    var laid = vec3<f32>(dot(P.gaze[0].xyz, delta), dot(P.gaze[1].xyz, delta), dot(P.gaze[2].xyz, delta));
    if (P.head.z == 2u) { laid = vec3<f32>(delta.x); }
    return vec4<f32>(grain_coded(base + laid), own.a);
}
