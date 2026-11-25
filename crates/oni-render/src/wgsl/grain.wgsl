// `grain`: grains on a jittered lattice anchored on the scan's full-resolution grid, each the soft union of four clouds;
// a pixel reads the film where the grains over it are centred, then each layer's own grains' deviations, through the gaze.

struct Params {
    // x, y: the photo's seed, added to every lattice index; z: the fields, the sampling one then one a layer.
    head: vec4<u32>,
    // The output centre to the scan's full grid, its x row then its y row; w: a grain's radius, then its clouds'.
    scan_x: vec4<f32>,
    scan_y: vec4<f32>,
    // That grid back to the output's pixels; w: how far a grain covering a pixel is centred.
    render_x: vec4<f32>,
    render_y: vec4<f32>,
    // Each layer's deviation, in shares of its window.
    deviation: vec4<f32>,
    // A negative's light on each layer's bare film, its pedestal added; w: that pedestal.
    bare: vec4<f32>,
    // Each layer's window in density; w: 1 on a negative, 0 on a positive read as its screen code.
    window: vec4<f32>,
    // Each channel's mix of the layers' deviations.
    gaze: array<vec4<f32>, 3>,
}

// PCG-4D: four words, exact on every device.
fn grain_hash(v0: vec4<u32>) -> vec4<u32> {
    var v = v0 * 1664525u + 1013904223u;
    v.x += v.y * v.w;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v.w += v.y * v.z;
    v = v ^ (v >> vec4<u32>(16u));
    v.x += v.y * v.w;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v.w += v.y * v.z;
    return v;
}

fn grain_byte(w: u32, i: u32) -> f32 {
    return (f32((w >> (8u * i)) & 0xffu) + 0.5) / 256.0;
}

fn grain_half(w: u32, i: u32) -> f32 {
    return (f32((w >> (16u * i)) & 0xffffu) + 0.5) / 65536.0;
}

fn grain_normal(w: u32) -> f32 {
    return (grain_byte(w, 0u) + grain_byte(w, 1u) + grain_byte(w, 2u) + grain_byte(w, 3u) - 2.0) * 1.7320508;
}

// A cloud's offset from its grain's centre, in grain radii.
fn grain_offset(w: u32) -> vec2<f32> {
    let a = grain_byte(w, 0u) + grain_byte(w, 1u) - 1.0;
    let b = grain_byte(w, 2u) + grain_byte(w, 3u) - 1.0;
    return vec2<f32>(a, b) * (0.32 / 0.4082483);
}

fn grain_profile(d: f32, rc: f32) -> f32 {
    let w = clamp((1.3 - d / rc) / 0.6, 0.0, 1.0);
    return w * w;
}

// A light's share of each window, or its screen code on a positive.
fn grain_shared(s: vec3<f32>) -> vec3<f32> {
    if (P.window.w > 0.5) {
        return -log2(max((s + P.bare.w) / P.bare.xyz, vec3<f32>(1e-30))) * 0.30102999566398120 / P.window.xyz;
    }
    return select(1.055 * pow(max(s, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055, 12.92 * s, s <= vec3<f32>(0.0031308));
}

fn grain_coded(s: vec3<f32>) -> vec3<f32> {
    if (P.window.w > 0.5) {
        return P.bare.xyz * exp2(-3.3219280948873623 * P.window.xyz * s) - P.bare.w;
    }
    return select(pow(max((s + 0.055) / 1.055, vec3<f32>(0.0)), vec3<f32>(2.4)), s / 12.92, s <= vec3<f32>(0.04045));
}

fn grain_px(p: vec2<i32>) -> vec4<f32> {
    let at = vec2<f32>(p) + 0.5;
    let own = load(p);
    let q = vec2<f32>(dot(P.scan_x.xyz, vec3<f32>(at, 1.0)), dot(P.scan_y.xyz, vec3<f32>(at, 1.0)));
    let r = P.scan_x.w;
    let rc = P.scan_y.w;
    let reach = P.render_x.w;
    let step = 0.7 * r;
    let lo = vec2<i32>(floor((q - reach) / step));
    let count = min(vec2<i32>(floor((q + reach) / step)) - lo + 1, vec2<i32>(8));
    var sample = vec3<f32>(0.0);
    var sampled = 0.0;
    var noise = vec3<f32>(0.0);
    var noised = vec3<f32>(0.0);
    for (var field = 0u; field < P.head.z; field++) {
        let lane = vec3<f32>(f32(field == 1u), f32(field == 2u), f32(field == 3u));
        for (var j = 0; j < count.y; j++) {
            for (var i = 0; i < count.x; i++) {
                let cell = lo + vec2<i32>(i, j);
                let a = grain_hash(vec4<u32>(bitcast<vec2<u32>>(cell) + P.head.xy, field, 0u));
                let centre = (vec2<f32>(cell) + vec2<f32>(grain_half(a.x, 0u), grain_half(a.x, 1u))) * step;
                let to = q - centre;
                if (length(to) > reach) { continue; }
                let b = grain_hash(a);
                let w = grain_profile(length(to - grain_offset(a.z) * r), rc) + grain_profile(length(to - grain_offset(a.w) * r), rc)
                    + grain_profile(length(to - grain_offset(b.x) * r), rc) + grain_profile(length(to - grain_offset(b.y) * r), rc);
                if (w <= 0.0) { continue; }
                if (field == 0u) {
                    let rp = vec2<f32>(dot(P.render_x.xyz, vec3<f32>(centre, 1.0)), dot(P.render_y.xyz, vec3<f32>(centre, 1.0)));
                    sample += w * grain_shared(load(vec2<i32>(floor(rp))).rgb);
                    sampled += w;
                } else {
                    noise += lane * (w * grain_normal(a.y) * dot(lane, P.deviation.xyz));
                    noised += lane * w;
                }
            }
        }
    }
    let base = select(grain_shared(own.rgb), sample / sampled, sampled > 0.05);
    var delta = select(vec3<f32>(0.0), noise / max(noised, vec3<f32>(1e-30)), noised > vec3<f32>(0.05));
    var laid = vec3<f32>(dot(P.gaze[0].xyz, delta), dot(P.gaze[1].xyz, delta), dot(P.gaze[2].xyz, delta));
    if (P.head.z == 2u) { laid = vec3<f32>(delta.x); }
    return vec4<f32>(grain_coded(base + laid), own.a);
}
