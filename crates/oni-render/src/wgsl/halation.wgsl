// `halation`: `amount` of the light past `threshold` moved from where it fell to its blur, so a wide
// highlight keeps its light and its edge glows; needs `HALATION_SOURCE`.

struct Params { head: vec4<u32>, dose: vec4<f32> }

fn halation_px(s: vec4<f32>, blurred: vec4<f32>) -> vec4<f32> {
    let own = halation_source(s, P.dose.y);
    return vec4<f32>(s.rgb + P.dose.x * (blurred.rgb - own.rgb), s.a);
}
