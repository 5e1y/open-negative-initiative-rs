// `grain`: a texture in stops of the film's light on a cubic B-spline lattice anchored on the scan's full-resolution grid,
// each pixel the field's mean over its own footprint, its share read off its light, its own colour deepened where the picture darkens.

struct Params {
    // x, y: the photo's seed, added to every lattice index, so each photo lays its own arrangement.
    head: vec4<u32>,
    // The output centre to the scan's full grid, its x row then its y row; w: one over the cell side,
    // then half the footprint in cells.
    row_x: vec4<f32>,
    row_y: vec4<f32>,
    // x: the stops a unit of the mean moves the light by; y, z: the table's first stop and its nodes a stop.
    dose: vec4<f32>,
    // x: the share of the blurred film a clump takes at the whole share; y: one over the field's spread
    // at full resolution, which a clump's edge is measured in; z: the share of it laid between clumps.
    mix: vec4<f32>,
    // A negative's light on each layer's bare film, its reading's pedestal added; w: that pedestal.
    bare: vec4<f32>,
    // Each layer's window over their mean; w: 1 on a negative, read as shares of its windows, 0 on a positive.
    layers: vec4<f32>,
    // A node's share, the next node's, its colour and the next one's, `GRAIN_NODES` of them: one row
    // read a pixel, no lane indexed.
    nodes: array<vec4<f32>, 64>,
}

// The table at the light of `s`: its share, then its colour, each a line between two nodes.
fn grain_node(s: vec3<f32>) -> vec2<f32> {
    let t = clamp((log2(max(dot(s, LUMA), 1e-30)) - P.dose.y) * P.dose.z, 0.0, 63.0);
    let i = min(floor(t), 62.0);
    let n = P.nodes[i32(i)];
    return n.xz + (n.yw - n.xz) * (t - i);
}

// ∫ B from −2 to `u`, B the cubic B-spline.
fn grain_ramp(u: f32) -> f32 {
    let a = min(abs(u), 2.0);
    let far = 2.0 - a;
    var tail = 0.5 - (4.0 * a - 2.0 * a * a * a + 0.75 * a * a * a * a) / 6.0;
    if (a >= 1.0) { tail = far * far * far * far / 24.0; }
    return select(tail, 1.0 - tail, u >= 0.0);
}

// Four 16-bit uniforms at a lattice node: PCG-2D, exact on every device.
fn grain_hash(i: vec2<i32>) -> vec4<f32> {
    var v = (bitcast<vec2<u32>>(i) + P.head.xy) * 1664525u + 1013904223u;
    // Two rounds written out: naga bounds every loop on Metal (force_loop_bounding), which blocks its
    // unrolling and kept the kernel 30 to 45 % slower.
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v = v ^ (v >> vec2<u32>(16u));
    v.x += v.y * 1664525u;
    v.y += v.x * 1664525u;
    v = v ^ (v >> vec2<u32>(16u));
    return (vec4<f32>(vec4<u32>(v.x & 0xffffu, v.x >> 16u, v.y & 0xffffu, v.y >> 16u)) + 0.5) * (1.0 / 65536.0);
}

// The light's standard normal: the four uniforms summed, so no transcendental is needed.
fn grain_normal(u: vec4<f32>) -> f32 {
    return (u.x + u.y + u.z + u.w - 2.0) * 1.7320508;
}

// `s` moved `a` stops, each channel's ratio to the luma raised to 2^(colour × `darkened`, the stops the picture
// darkens): its own hue deepened, a grey kept grey, its luma the grain's; a channel at or under zero is kept.
fn grain_laid(s: vec3<f32>, a: f32, colour: f32, darkened: f32) -> vec3<f32> {
    let k = exp2(a);
    let darker = colour * max(darkened, 0.0);
    let y = dot(s, LUMA);
    if (darker <= 0.0 || y <= 0.0) { return s * k; }
    let r = s / y;
    let p = select(r, exp2(exp2(darker) * log2(r)), r > vec3<f32>(0.0));
    let q = dot(p, LUMA);
    if (q <= 0.0) { return s * k; }
    return p * (k * y / q);
}

// `at` is the pixel's centre in the graph's pixels, `b` the film blurred under a grain; a light given no share is kept.
fn grain_px(s: vec4<f32>, b: vec4<f32>, at: vec2<f32>) -> vec4<f32> {
    let node = grain_node(s.rgb);
    if (node.x <= 0.0) { return s; }
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
            row += wx[c] * grain_normal(grain_hash(first + vec2<i32>(c, r)));
        }
        field += wy[r] * row;
    }
    // Inside a clump the film gives way to its blur, so detail finer than a grain follows the grain; a haze of it between.
    let clump = smoothstep(0.5, 1.5, abs(field) * P.mix.y);
    let w = P.mix.x * node.x * (P.mix.z + (1.0 - P.mix.z) * clump);
    let base = s.rgb + w * (b.rgb - s.rgb);
    let a = P.dose.x * node.x * field;
    if (P.layers.w > 0.5) {
        // A negative: each layer read as its share of its window, a lighter film a darker picture, then coded back.
        let read = pow(max((base + P.bare.w) / P.bare.xyz, vec3<f32>(1e-30)), 1.0 / P.layers.xyz);
        let laid = grain_laid(read, a, node.y, a);
        return vec4<f32>(P.bare.xyz * pow(max(laid, vec3<f32>(0.0)), P.layers.xyz) - P.bare.w, s.a);
    }
    return vec4<f32>(grain_laid(base, a, node.y, -a), s.a);
}
