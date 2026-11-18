// `halation_source`: the light past `threshold` a pixel sends back through the film's base, its red
// layer's most, read by the blur's taps and by `halation_px`.

const HALATION_COLOUR: vec3<f32> = vec3<f32>(1.0, 0.3, 0.0);

fn halation_source(s: vec4<f32>, threshold: f32) -> vec4<f32> {
    return vec4<f32>(max(s.rgb - threshold, vec3<f32>(0.0)) * HALATION_COLOUR, 0.0);
}
