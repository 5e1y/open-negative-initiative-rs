@group(0) @binding(0) var canvas: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> bins: array<atomic<u32>, 768>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(canvas);
    let p = id.xy * 2u;
    if (p.x >= size.x || p.y >= size.y) { return; }
    let v = textureLoad(canvas, vec2<i32>(p), 0);
    for (var c = 0u; c < 3u; c++) {
        atomicAdd(&bins[u32(round(clamp(v[c], 0.0, 1.0) * 255.0)) * 3u + c], 1u);
    }
}
