struct Params {
    rows: array<vec4<f32>, 3>,
    width: u32,
    height: u32,
    seed: u32,
    dither: f32,
    low: vec4<f32>,
    high: vec4<f32>,
    tint: vec4<f32>,
    origin: vec2<f32>,
    stretch: f32,
    canvas_height: f32,
    phase: f32,
    period: f32,
    duty: f32,
    zebra: f32,
    ground: f32,
    pad0: f32,
    pad1: f32,
    pad2: f32,
};

@group(0) @binding(0) var<uniform> k: Params;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var canvas: texture_storage_2d<rgba8unorm, write>;

fn pcg(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn unit(v: u32) -> f32 {
    return f32(v >> 8u) / 16777216.0;
}

fn transfer(v: f32) -> f32 {
    if (v <= 0.0031308) { return 12.92 * v; }
    return 1.055 * pow(v, 1.0 / 2.4) - 0.055;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= k.width || id.y >= k.height) { return; }
    var p = textureLoad(source, vec2<i32>(i32(id.x), i32(id.y)), 0);
    let alpha = clamp(p.a, 0.0, 1.0);
    if (k.zebra > 0.5) {
        var beyond = false;
        for (var c = 0u; c < 3u; c++) {
            let shown = dot(k.rows[c].xyz, p.xyz);
            beyond = beyond || shown < k.low[c] || shown > k.high[c];
        }
        let cx = k.origin.x + (f32(id.x) + 0.5) * k.stretch;
        let cy = k.canvas_height - (k.origin.y + (f32(id.y) + 0.5) * k.stretch);
        let t = cx + cy + k.phase;
        if (beyond && t - floor(t / k.period) * k.period < k.duty * k.period) { p = k.tint; }
    }
    let h = pcg(id.x ^ pcg(id.y ^ pcg(k.seed)));
    var out = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    for (var c = 0u; c < 3u; c++) {
        let r = k.rows[c];
        let linear = clamp((r.x * p.x + r.y * p.y + r.z * p.z) * alpha + k.ground * (1.0 - alpha), 0.0, 1.0);
        let d = (unit(pcg(h + 2u * c)) + unit(pcg(h + 2u * c + 1u)) - 1.0) * k.dither;
        // Quantised here, so the texture's own conversion only stores an exact level.
        out[c] = clamp(round(transfer(linear) * 255.0 + d), 0.0, 255.0) / 255.0;
    }
    textureStore(canvas, vec2<u32>(id.x, id.y), out);
}
