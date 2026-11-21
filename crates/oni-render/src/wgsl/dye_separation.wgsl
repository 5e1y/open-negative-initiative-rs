// The dye separation, operation for operation as `cpu::dye_separation`: the scan's densities above the bare
// film, on the camera's own channels, solved for the stock's dye amounts by Newton's steps; alpha kept.

struct Params {
    head: vec4<u32>,
    gain_pedestal: vec4<f32>,
    black_guard: vec4<f32>,
    base_dose: vec4<f32>,
    bare: vec4<f32>,
    to_camera: array<vec4<f32>, 3>,
    matrix: array<vec4<f32>, 3>,
    tone_low: vec4<f32>,
    tone_step: vec4<f32>,
    dyes: array<vec4<f32>, 64>,
    weights: array<vec4<f32>, 64>,
    tone: array<vec4<f32>, 128>,
}

const DYE_STEPS: u32 = 3u;
const DYE_STEP_BOUND: f32 = 0.5;
const DYE_LIGHT_FLOOR: f32 = 1e-12;
const DYE_TONE_NODES: f32 = 128.0;

fn dyes_times(m: array<vec4<f32>, 3>, v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(dot(m[0].xyz, v), dot(m[1].xyz, v), dot(m[2].xyz, v));
}

fn dye_separation_px(s: vec4<f32>) -> vec4<f32> {
    let guard = P.black_guard.w;
    let t = (s.rgb + P.gain_pedestal.w) * P.gain_pedestal.xyz;
    let above = -log(max(t, vec3<f32>(guard))) * LOG10_E - P.black_guard.xyz;
    let light = P.bare.xyz * exp2(-3.3219280948873623 * above);
    let target_ = -log(max(dyes_times(P.to_camera, light), vec3<f32>(DYE_LIGHT_FLOOR))) * LOG10_E;
    var amounts = dyes_times(P.matrix, target_ - P.base_dose.xyz);
    for (var step = 0u; step < DYE_STEPS; step = step + 1u) {
        var light = vec3<f32>(0.0);
        var r = vec3<f32>(0.0);
        var g = vec3<f32>(0.0);
        var b = vec3<f32>(0.0);
        for (var k = 0u; k < P.head.x; k = k + 1u) {
            let dye = P.dyes[k];
            let passed = exp2(-3.3219280948873623 * (dot(amounts, dye.xyz) + dye.w)) * P.weights[k].rgb;
            light = light + passed;
            r = r + passed.r * dye.xyz;
            g = g + passed.g * dye.xyz;
            b = b + passed.b * dye.xyz;
        }
        let held = max(light, vec3<f32>(DYE_LIGHT_FLOOR));
        let miss = -log(held) * LOG10_E - target_;
        r = r / held.r;
        g = g / held.g;
        b = b / held.b;
        let ca = g.y * b.z - g.z * b.y;
        let cb = g.z * b.x - g.x * b.z;
        let cc = g.x * b.y - g.y * b.x;
        let det = r.x * ca + r.y * cb + r.z * cc;
        var move_ = dyes_times(P.matrix, miss);
        if (abs(det) > 1e-12) {
            let i0 = vec3<f32>(ca, r.z * b.y - r.y * b.z, r.y * g.z - r.z * g.y);
            let i1 = vec3<f32>(cb, r.x * b.z - r.z * b.x, r.z * g.x - r.x * g.z);
            let i2 = vec3<f32>(cc, r.y * b.x - r.x * b.y, r.x * g.y - r.y * g.x);
            move_ = vec3<f32>(dot(i0, miss), dot(i1, miss), dot(i2, miss)) / det;
        }
        amounts = amounts - clamp(move_, vec3<f32>(-DYE_STEP_BOUND), vec3<f32>(DYE_STEP_BOUND));
    }
    let dosed = above + P.base_dose.w * (amounts - above);
    let f = (dosed - P.tone_low.xyz) / P.tone_step.xyz;
    let i = clamp(floor(f), vec3<f32>(0.0), vec3<f32>(DYE_TONE_NODES - 2.0));
    let along = f - i;
    let at = vec3<u32>(i);
    let toned = vec3<f32>(
        mix(P.tone[at.x].x, P.tone[at.x + 1u].x, along.x),
        mix(P.tone[at.y].y, P.tone[at.y + 1u].y, along.y),
        mix(P.tone[at.z].z, P.tone[at.z + 1u].z, along.z),
    );
    return vec4<f32>(exp2(-3.3219280948873623 * toned), s.a);
}
