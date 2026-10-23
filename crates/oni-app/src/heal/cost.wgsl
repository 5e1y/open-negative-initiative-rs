// `correctionCost`: one cost per candidate offset, `cost_map` on the CPU texel for texel. The source is
// the search region's pixels, row-major y down; every point is in the frame, y up.

struct Params {
    region: vec4<i32>,
    frame: vec2<f32>,
    centre: vec2<f32>,
    half_size: vec2<f32>,
    origin: vec2<f32>,
    hole: vec4<f32>,
    extent: vec4<f32>,
    step: f32,
    min_radius: f32,
    grid_n: u32,
    size: u32,
}

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> src: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> costs: array<f32>;

fn infinite() -> f32 {
    return bitcast<f32>(0x7f800000u);
}

// `Region::texel`: clear past the frame, edge-clamped inside the region.
fn texel(col: i32, row_up: i32) -> vec3<f32> {
    let frame_h = i32(P.frame.y);
    if (col < 0 || row_up < 0 || f32(col) >= P.frame.x || f32(row_up) >= P.frame.y) {
        return vec3<f32>(0.0);
    }
    let row_down = frame_h - 1 - row_up;
    let c = clamp(col - P.region.x, 0, P.region.z - 1);
    let r = clamp(row_down - P.region.y, 0, P.region.w - 1);
    return src[u32(r * P.region.z + c)].rgb;
}

// `Region::sample`: bilinear, texel centres on the half pixel.
fn sampled(p: vec2<f32>) -> vec3<f32> {
    let u = p - vec2<f32>(0.5);
    let base = floor(u);
    let t = u - base;
    let x = i32(base.x);
    let y = i32(base.y);
    let low = mix(texel(x, y), texel(x + 1, y), t.x);
    let high = mix(texel(x, y + 1), texel(x + 1, y + 1), t.x);
    return mix(low, high, t.y);
}

fn inside(p: vec2<f32>) -> bool {
    let h = P.hole;
    return p.x >= h.x && p.x <= h.x + h.z && p.y >= h.y && p.y <= h.y + h.w;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let index = id.y * groups.x * 64u + id.x;
    if (index >= P.size * P.size) {
        return;
    }
    let offset = P.origin + (vec2<f32>(f32(index % P.size), f32(index / P.size)) + 0.5) * P.step;
    let cand = P.centre + offset;
    let e = P.extent;
    // A candidate overlapping its destination, or whose ring leaves the photo, scores no match.
    if (length(offset) < P.min_radius
        || cand.x - P.half_size.x < e.x || cand.x + P.half_size.x > e.x + e.z
        || cand.y - P.half_size.y < e.y || cand.y + P.half_size.y > e.y + e.w) {
        costs[index] = infinite();
        return;
    }
    let n = P.grid_n;
    var accum = vec3<f32>(0.0);
    var counted = 0u;
    for (var j = 0u; j < n; j++) {
        for (var k = 0u; k < n; k++) {
            let cell = ((vec2<f32>(f32(k), f32(j)) + 0.5) / f32(n) * 2.0 - 1.0) * P.half_size;
            let dest = P.centre + cell;
            let source = cand + cell;
            // A cell inside the hole, on either side, compares the defect rather than structure.
            if (inside(dest) || inside(source)) {
                continue;
            }
            let d = sampled(dest);
            let diff = d - sampled(source);
            accum += diff * diff / (d * d + vec3<f32>(0.001));
            counted++;
        }
    }
    costs[index] = select((accum.x + accum.y + accum.z) / f32(counted), infinite(), counted == 0u);
}
