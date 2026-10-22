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
