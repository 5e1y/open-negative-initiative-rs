struct Measure {
    to: array<vec4<f32>, 3>,
    back: array<vec4<f32>, 3>,
    // The counted copy's columns and rows, then the picture's.
    grid: vec4<u32>,
};

@group(0) @binding(0) var<uniform> m: Measure;
@group(0) @binding(1) var picture: texture_2d<f32>;
// Red, green, blue and luminance of each bin, side by side.
@group(0) @binding(2) var<storage, read_write> bins: array<atomic<u32>, 4096>;

const BINS: u32 = 1024u;
const LUMA = vec3<f32>(0.2627, 0.6780, 0.0593);

fn nearest(cell: u32, cells: u32, size: u32) -> u32 {
    return min(u32((f32(cell) + 0.5) * f32(size) / f32(cells)), size - 1u);
}

fn bin(v: f32) -> u32 {
    return min(u32(clamp(v, 0.0, 1.0) * f32(BINS)), BINS - 1u);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= m.grid.x || id.y >= m.grid.y) { return; }
    let at = vec2<i32>(i32(nearest(id.x, m.grid.x, m.grid.z)), i32(nearest(id.y, m.grid.y, m.grid.w)));
    let p = textureLoad(picture, at, 0).xyz;
    let shown = clamp(vec3<f32>(dot(m.to[0].xyz, p), dot(m.to[1].xyz, p), dot(m.to[2].xyz, p)), vec3<f32>(0.0), vec3<f32>(1.0));
    let v = vec3<f32>(dot(m.back[0].xyz, shown), dot(m.back[1].xyz, shown), dot(m.back[2].xyz, shown));
    for (var c = 0u; c < 3u; c++) {
        atomicAdd(&bins[bin(v[c]) * 4u + c], 1u);
    }
    atomicAdd(&bins[bin(dot(LUMA, v)) * 4u + 3u], 1u);
}
