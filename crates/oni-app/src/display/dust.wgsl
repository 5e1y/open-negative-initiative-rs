// Auto De-dust's veil at a notch of the slider: a map texel holds the first notch its pixels show from, so
// what the notch has not reached takes the veil and the rest stays clear. Straight alpha, as the interface reads a texture.

struct Params {
    veil: vec4<f32>,
    notch: u32,
    pad0: u32,
    pad1: u32,
    pad2: u32,
};

@group(0) @binding(0) var<uniform> k: Params;
@group(0) @binding(1) var map: texture_2d<u32>;
@group(0) @binding(2) var mask: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(map);
    if (id.x >= size.x || id.y >= size.y) {
        return;
    }
    let t = textureLoad(map, vec2<i32>(id.xy), 0).r;
    var alpha = k.veil.a;
    if (t <= k.notch) {
        alpha = 0.0;
    }
    textureStore(mask, vec2<i32>(id.xy), vec4<f32>(k.veil.rgb, alpha));
}
