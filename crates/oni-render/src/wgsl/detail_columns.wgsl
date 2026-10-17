// `detailColumns`: the columns, then chroma denoise, luma untouched so the grain stays, and an unsharp
// mask in stops held by `halo` inside the extremes of a disk. Rows are read clamped vertically.

struct Params {
    head: vec4<u32>,
    sigma_reach: vec4<f32>,
    disk_force_amount_halo: vec4<f32>,
}

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> rows: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> dest: array<vec4<f32>>;

// Row `y` of the rows pass, column `x` of the source; the rows run `disk` wider each side.
fn row(x: i32, y: i32) -> vec4<f32> {
    let h = i32(P.head.z);
    let disk = i32(P.disk_force_amount_halo.x);
    return rows[u32(clamp(y, 0, h - 1)) * P.head.w + u32(x + disk)];
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    let x = i32(i % P.head.y);
    let y = i32(i / P.head.y);
    let sigma = P.sigma_reach.xy;
    let reach = P.sigma_reach.zw;
    let disk = P.disk_force_amount_halo.x;
    let force = P.disk_force_amount_halo.y;
    let amount = P.disk_force_amount_halo.z;
    let halo = P.disk_force_amount_halo.w;
    let s = src_px[i];
    var chroma_taps = taps(sigma.x, reach.x);
    var luma_taps = taps(sigma.y, reach.y);
    let here = row(x, y);
    var chroma = tap_centre(chroma_taps) * here.rg;
    var luma = tap_centre(luma_taps) * here.b;
    let k = i32(max(reach.x, reach.y));
    for (var d = 1; d <= k; d++) {
        let a = row(x, y + d);
        let b = row(x, y - d);
        if (f32(d) <= reach.x) { chroma += tap_next(&chroma_taps, d) * (a.rg + b.rg); }
        if (f32(d) <= reach.y) { luma += tap_next(&luma_taps, d) * (a.b + b.b); }
    }
    var rgb = s.rgb;
    if (force > 0.0) {
        let yl = dot(rgb, LUMA);
        let blurred = vec3<f32>(chroma, -(LUMA.r * chroma.x + LUMA.g * chroma.y) / LUMA.b);
        var mixed = mix(rgb - yl, blurred, vec3<f32>(force));
        mixed = mixed - dot(mixed, LUMA);
        rgb = yl + mixed;
    }
    if (amount > 0.0) {
        let yl = max(dot(rgb, LUMA), 0.0);
        let l = log2(yl + SHARPEN_TOE);
        var sharpened = l + (l - log2(max(luma, 0.0) + SHARPEN_TOE)) * amount;
        if (halo > 0.0) {
            let r = i32(disk);
            var lowest = infinity();
            var highest = -infinity();
            for (var dy = -r; dy <= r; dy++) {
                let span = i32(floor(sqrt(f32(r * r - dy * dy))));
                for (var dx = -span; dx <= span; dx++) {
                    let v = row(x + dx, y + dy).a;
                    lowest = min(lowest, v);
                    highest = max(highest, v);
                }
            }
            let held = clamp(sharpened, log2(max(lowest, 0.0) + SHARPEN_TOE), log2(max(highest, 0.0) + SHARPEN_TOE));
            sharpened = mix(sharpened, held, halo);
        }
        rgb = rgb + (exp2(sharpened) - SHARPEN_TOE - yl);
    }
    dest[i] = vec4<f32>(rgb, s.a);
}
