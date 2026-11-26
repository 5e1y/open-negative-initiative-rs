// `halation_laid`: each layer's exposure joined by `dose` of its glow, the glow read bilinearly off the film's plate,
// where `row_x` and `row_y` map the pixel's centre; alpha kept.

struct Params { head: vec4<u32>, dose: vec4<f32>, saturation: vec4<f32>, row_x: vec4<f32>, row_y: vec4<f32> }

fn halation_laid_px(p: vec2<i32>) -> vec4<f32> {
    let e = load(p);
    let c = vec2<f32>(p) + 0.5;
    let at = vec2<f32>(dot(P.row_x.xy, c) + P.row_x.z, dot(P.row_y.xy, c) + P.row_y.z) - 0.5;
    let f = floor(at);
    let t = at - f;
    let q = vec2<i32>(f);
    let top = mix(load1(q), load1(q + vec2<i32>(1, 0)), t.x);
    let bottom = mix(load1(q + vec2<i32>(0, 1)), load1(q + vec2<i32>(1, 1)), t.x);
    let glow = mix(top, bottom, t.y);
    return vec4<f32>(e.rgb + P.dose.rgb * glow.rgb, e.a);
}
