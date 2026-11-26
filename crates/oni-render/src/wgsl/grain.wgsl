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
    // The scan's grid onto the level of the layers' field read, its x row then its y row, twice.
    plate: array<vec4<f32>, 4>,
    // x: the box's half width in texels, zero for a bilinear read; y: the level's spread.
    blend: vec4<f32>,
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

// Where `at` lies on the scan's grid and the lattice cells whose grains may cover it: the first and how many.
struct GrainCells { q: vec2<f32>, lo: vec2<i32>, count: vec2<i32> }

fn grain_cells(at: vec2<f32>) -> GrainCells {
    var c: GrainCells;
    c.q = vec2<f32>(dot(P.scan_x.xyz, vec3<f32>(at, 1.0)), dot(P.scan_y.xyz, vec3<f32>(at, 1.0)));
    let step = 0.7 * P.scan_x.w;
    let reach = P.render_x.w;
    c.lo = vec2<i32>(floor((c.q - reach) / step));
    c.count = min(vec2<i32>(floor((c.q + reach) / step)) - c.lo + 1, vec2<i32>(8));
    return c;
}

// Field `field`'s grain of cell `cell` over `q`: its first hash word, its centre in `xy`, its weight in `z`, zero
// where none of its clouds reaches.
fn grain_over(cell: vec2<i32>, field: u32, q: vec2<f32>, a: ptr<function, vec4<u32>>) -> vec3<f32> {
    let r = P.scan_x.w;
    let rc = P.scan_y.w;
    let reach = P.render_x.w;
    let step = 0.7 * r;
    *a = grain_hash(vec4<u32>(bitcast<vec2<u32>>(cell) + P.head.xy, field, 0u));
    let centre = (vec2<f32>(cell) + vec2<f32>(grain_half((*a).x, 0u), grain_half((*a).x, 1u))) * step;
    let to = q - centre;
    if (length(to) > reach) { return vec3<f32>(centre, 0.0); }
    let b = grain_hash(*a);
    let w = grain_profile(length(to - grain_offset((*a).z) * r), rc) + grain_profile(length(to - grain_offset((*a).w) * r), rc)
        + grain_profile(length(to - grain_offset(b.x) * r), rc) + grain_profile(length(to - grain_offset(b.y) * r), rc);
    return vec3<f32>(centre, w);
}

// Each layer's grains at `p`: their draws' mean through the grains over it, in standard normals; the film, the
// deviations and the gaze play no part, so the field holds while a slider moves them.
fn grain_field_px(p: vec2<i32>) -> vec4<f32> {
    let c = grain_cells(vec2<f32>(p) + 0.5);
    var out = vec3<f32>(0.0);
    for (var field = 1u; field < P.head.z; field++) {
        var noise = 0.0;
        var noised = 0.0;
        for (var j = 0; j < c.count.y; j++) {
            for (var i = 0; i < c.count.x; i++) {
                var a: vec4<u32>;
                let g = grain_over(c.lo + vec2<i32>(i, j), field, c.q, &a);
                if (g.z <= 0.0) { continue; }
                noise += g.z * grain_normal(a.y);
                noised += g.z;
            }
        }
        out[field - 1u] = select(0.0, noise / noised, noised > 0.05);
    }
    return vec4<f32>(out, 1.0);
}
