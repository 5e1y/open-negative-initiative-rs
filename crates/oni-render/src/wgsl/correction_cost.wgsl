// `correctionCost`: one output texel per candidate offset, the SSD plus the offset itself. The
// source is sampled bilinearly in Core Image's frame, transparent outside its extent.

struct Params {
    head: vec4<u32>,
    dest_centre_half_size: vec4<f32>,
    window_origin_step_grid: vec4<f32>,
    extent: vec4<f32>,
    hole: vec4<f32>,
    min_radius_source_origin: vec4<f32>,
}

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> dest: array<vec4<f32>>;

fn texel(x: i32, y: i32) -> vec3<f32> {
    let w = i32(P.head.z);
    let h = i32(P.head.w);
    if (x < 0 || y < 0 || x >= w || y >= h) { return vec3<f32>(0.0); }
    return src_px[u32(y * w + x)].rgb;
}

fn sampled(p: vec2<f32>) -> vec3<f32> {
    let origin = P.min_radius_source_origin.yz;
    let f = vec2<f32>(p.x - origin.x - 0.5, f32(P.head.w) - (p.y - origin.y) - 0.5);
    let base = floor(f);
    let t = f - base;
    let x = i32(base.x);
    let y = i32(base.y);
    let top_row = mix(texel(x, y), texel(x + 1, y), t.x);
    let bottom_row = mix(texel(x, y + 1), texel(x + 1, y + 1), t.x);
    return mix(top_row, bottom_row, t.y);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    let grid = P.head.y;
    let dest_centre = P.dest_centre_half_size.xy;
    let half_size = P.dest_centre_half_size.zw;
    let step = P.window_origin_step_grid.z;
    let min_radius = P.min_radius_source_origin.x;
    let extent_min = P.extent.xy;
    let extent_max = P.extent.zw;
    let hole = P.hole;
    let at = ci_coord(i % grid, i / grid, vec2<f32>(0.0), grid);
    let offset = P.window_origin_step_grid.xy + at * step;
    let cand = dest_centre + offset;
    // A candidate overlapping its destination, or whose ring leaves the photo, scores a spurious match.
    if (length(offset) < min_radius
        || cand.x - half_size.x < extent_min.x || cand.x + half_size.x > extent_max.x
        || cand.y - half_size.y < extent_min.y || cand.y + half_size.y > extent_max.y) {
        dest[i] = vec4<f32>(infinity(), offset, 1.0);
        return;
    }
    let n = i32(P.window_origin_step_grid.w);
    var accum = vec3<f32>(0.0);
    var counted = 0;
    for (var j = 0; j < n; j++) {
        for (var k = 0; k < n; k++) {
            let t = (vec2<f32>(f32(k), f32(j)) + 0.5) / f32(n) * 2.0 - 1.0;
            let cell = t * half_size;
            let dest_pos = dest_centre + cell;
            let src_pos = cand + cell;
            // A cell inside the hole, on either side, compares the defect rather than structure.
            let dest_in_hole = dest_pos.x >= hole.x && dest_pos.x <= hole.z && dest_pos.y >= hole.y && dest_pos.y <= hole.w;
            let src_in_hole = src_pos.x >= hole.x && src_pos.x <= hole.z && src_pos.y >= hole.y && src_pos.y <= hole.w;
            if (dest_in_hole || src_in_hole) { continue; }
            let d = sampled(dest_pos);
            let s = sampled(src_pos);
            let diff = d - s;
            // Each channel's error over the destination's own squared amplitude, or the brightest part decides.
            accum += (diff * diff) / (d * d + vec3<f32>(0.001));
            counted++;
        }
    }
    if (counted == 0) {
        dest[i] = vec4<f32>(infinity(), offset, 1.0);
        return;
    }
    dest[i] = vec4<f32>((accum.x + accum.y + accum.z) / f32(counted), offset, 1.0);
}
