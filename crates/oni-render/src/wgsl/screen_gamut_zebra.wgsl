// `screenGamutZebra`: thin diagonal stripes over the pixels the screen clips, `shown` being the
// picture in the display's own encoding, unclamped.

struct Params {
    head: vec4<u32>,
    origin: vec4<f32>,
    tint: vec4<f32>,
    period_duty_phase_tolerance: vec4<f32>,
}

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> picture: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> shown: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read_write> dest: array<vec4<f32>>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let i = flat_index(id, groups);
    if (i >= P.head.x) { return; }
    let z = P.period_duty_phase_tolerance;
    let seen = shown[i].rgb;
    let beyond = any(seen < vec3<f32>(-z.w)) || any(seen > vec3<f32>(1.0 + z.w));
    if (!beyond) {
        dest[i] = picture[i];
        return;
    }
    let p = ci_coord(i % P.head.y, i / P.head.y, P.origin.xy, P.head.z);
    dest[i] = select(picture[i], P.tint, (p.x + p.y + z.z) % z.x < z.y * z.x);
}
