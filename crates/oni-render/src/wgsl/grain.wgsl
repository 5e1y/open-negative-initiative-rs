// `grain`: a monochrome texture in stops on a cubic B-spline lattice anchored on the scan's
// full-resolution grid, each pixel the field's mean over its own footprint.

struct Params {
    head: vec4<u32>,
    // The output centre to the scan's full grid, its x row then its y row; w: one over the cell side,
    // then half the footprint in cells.
    row_x: vec4<f32>,
    row_y: vec4<f32>,
    dose: vec4<f32>,
}

// ∫ B from −2 to `u`, B the cubic B-spline.
fn grain_ramp(u: f32) -> f32 {
    let a = min(abs(u), 2.0);
    let far = 2.0 - a;
    var tail = 0.5 - (4.0 * a - 2.0 * a * a * a + 0.75 * a * a * a * a) / 6.0;
    if (a >= 1.0) { tail = far * far * far * far / 24.0; }
    return select(tail, 1.0 - tail, u >= 0.0);
}

// A standard normal at a lattice node: PCG-2D, four 16-bit uniforms summed, exact on every device.
fn grain_normal(i: vec2<i32>) -> f32 {
    var v = bitcast<vec2<u32>>(i) * 1664525u + 1013904223u;
    // Two rounds written out: naga bounds every loop on Metal (force_loop_bounding), which blocks its
    // unrolling and kept the kernel 30 to 45 % slower.
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v = v ^ (v >> vec2<u32>(16u));
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v = v ^ (v >> vec2<u32>(16u));
    let u = (vec4<f32>(vec4<u32>(v.x & 0xffffu, v.x >> 16u, v.y & 0xffffu, v.y >> 16u)) + 0.5) * (1.0 / 65536.0);
    return (u.x + u.y + u.z + u.w - 2.0) * 1.7320508;
}

// `at` is the pixel's centre in the graph's pixels.
fn grain_px(s: vec4<f32>, at: vec2<f32>) -> vec4<f32> {
    let q = vec2<f32>(P.row_x.x * at.x + P.row_x.y * at.y + P.row_x.z, P.row_y.x * at.x + P.row_y.y * at.y + P.row_y.z) * P.row_x.w;
    let lo = q - P.row_y.w;
    let hi = q + P.row_y.w;
    let span = hi - lo;
    let first = vec2<i32>(floor(lo)) - 1;
    let count = min(vec2<i32>(ceil(hi)) + 2 - first, vec2<i32>(8));
    var wx: array<f32, 8>;
    var wy: array<f32, 8>;
    for (var j = 0; j < count.x; j++) {
        let i = f32(first.x + j);
        wx[j] = (grain_ramp(hi.x - i) - grain_ramp(lo.x - i)) / span.x;
    }
    for (var j = 0; j < count.y; j++) {
        let i = f32(first.y + j);
        wy[j] = (grain_ramp(hi.y - i) - grain_ramp(lo.y - i)) / span.y;
    }
    var field = 0.0;
    for (var r = 0; r < count.y; r++) {
        var row = 0.0;
        for (var c = 0; c < count.x; c++) {
            row += wx[c] * grain_normal(first + vec2<i32>(c, r));
        }
        field += wy[r] * row;
    }
    return vec4<f32>(s.rgb * exp2(P.dose.x * field), s.a);
}
