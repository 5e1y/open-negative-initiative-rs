// `detailRows`: the chroma's two free channels and the luma blurred along x, then the luma itself.
// Output: the source widened by `disk` each side, read clamped to its edges as `clampedToExtent`.

struct Params {
    head: vec4<u32>,
    sigma_reach: vec4<f32>,
    disk_force_amount_halo: vec4<f32>,
}

// Column `x`, row `y` of the source, read through `detail_source(x, y)`, which clamps to its edges.
fn detail_rows_px(x: i32, y: i32) -> vec4<f32> {
    let sigma = P.sigma_reach.xy;
    let reach = P.sigma_reach.zw;
    var chroma_taps = taps(sigma.x, reach.x);
    var luma_taps = taps(sigma.y, reach.y);
    let s = detail_source(x, y);
    let centre = dot(s, LUMA);
    var chroma = tap_centre(chroma_taps) * (s.rg - centre);
    var luma = tap_centre(luma_taps) * centre;
    let k = i32(max(reach.x, reach.y));
    for (var d = 1; d <= k; d++) {
        let a = detail_source(x + d, y);
        let b = detail_source(x - d, y);
        let ya = dot(a, LUMA);
        let yb = dot(b, LUMA);
        if (f32(d) <= reach.x) { chroma += tap_next(&chroma_taps, d) * (a.rg - ya + b.rg - yb); }
        if (f32(d) <= reach.y) { luma += tap_next(&luma_taps, d) * (ya + yb); }
    }
    return vec4<f32>(chroma, luma, centre);
}

// MARK: - Buffers

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> rows: array<vec4<f32>>;

fn detail_source(x: i32, y: i32) -> vec3<f32> {
    let w = i32(P.head.w);
    return src_px[u32(y) * P.head.w + u32(clamp(x, 0, w - 1))].rgb;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    let wide = P.head.y;
    rows[i] = detail_rows_px(i32(i % wide) - i32(P.disk_force_amount_halo.x), i32(i / wide));
}
