// The stage's picture finished for the screen: the glow of its brightest lights gathered small and spread wide,
// the veil a page's words stand on, the film's curve and a dither against banding.

struct Finish {
    // xy the picture's size, z the time, w the pass: 0 gathers the lights, 1 spreads them, 2 finishes.
    view: vec4<f32>,
    // x where the words' veil begins, in pixels (past the picture: none), y its depth, z the glow's strength,
    // w how far a spreading pass reaches, in picture heights.
    veil: vec4<f32>,
    // x where the footer's band begins, in pixels: the stage goes dark behind the window's keys.
    foot: vec4<f32>,
}

@group(0) @binding(0) var<uniform> finish: Finish;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var glow: texture_2d<f32>;
@group(0) @binding(3) var smooth_sampler: sampler;
@group(0) @binding(4) var small: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var screen: texture_storage_2d<rgba8unorm, write>;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.1030));
    let r = q + dot(q, q.yx + 33.33);
    return fract((r.x + r.y) * r.x);
}

// The film's curve: a shoulder past the whites, so the foils' brights roll off without clipping.
fn developed(c: vec3<f32>) -> vec3<f32> {
    let x = max(c * 1.25, vec3<f32>(0.0));
    return pow((x * (1.0 + x / 9.0)) / (1.0 + x), vec3<f32>(1.0 / 2.2));
}

// The picture at `uv` as the screen shows it: its light, the glow about its brightest parts, developed.
fn shown(uv: vec2<f32>) -> vec3<f32> {
    let lit = textureSampleLevel(source, smooth_sampler, uv, 0.0).rgb;
    let about = textureSampleLevel(glow, smooth_sampler, uv, 0.0).rgb;
    return developed(lit + about * finish.veil.z);
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = finish.view.xy;
    let pixel = vec2<f32>(id.xy) + 0.5;
    if finish.view.w < 1.5 {
        // The glow's two passes work on a picture a quarter the size.
        let quarter = ceil(size / 4.0);
        if pixel.x > quarter.x || pixel.y > quarter.y {
            return;
        }
        let uv = pixel / quarter;
        var sum = vec3<f32>(0.0);
        if finish.view.w < 0.5 {
            // What shines past white, each quarter pixel the mean of the sixteen it covers.
            for (var k = 0; k < 4; k++) {
                let off = (vec2<f32>(f32(k % 2), f32(k / 2)) - 0.5) * 2.0 / size;
                let lit = textureSampleLevel(source, smooth_sampler, uv + off, 0.0);
                let l = luma(lit.rgb);
                sum += lit.rgb * lit.a * max(l - 1.6, 0.0) / max(l, 0.001);
            }
            textureStore(small, vec2<i32>(id.xy), vec4<f32>(min(sum / 4.0, vec3<f32>(12.0)), 1.0));
            return;
        }
        // Spread on a spiral of taps, nearer ones weighing more: a soft, wide halo.
        var weight = 0.0;
        for (var k = 0; k < 24; k++) {
            let far = sqrt((f32(k) + 0.5) / 24.0);
            let turn = f32(k) * 2.39996;
            let off = vec2<f32>(cos(turn), sin(turn)) * far * finish.veil.w * vec2<f32>(size.y / size.x, 1.0);
            let w = 1.0 - 0.75 * far;
            sum += textureSampleLevel(glow, smooth_sampler, uv + off, 0.0).rgb * w;
            weight += w;
        }
        textureStore(small, vec2<i32>(id.xy), vec4<f32>(sum / weight, 1.0));
        return;
    }
    if pixel.x > size.x || pixel.y > size.y {
        return;
    }
    let uv = pixel / size;
    var colour = shown(uv);
    // The lens darkens toward its corners; a page's words stand on a veil that fades into the stage.
    let off = (pixel - 0.5 * size) / size.y;
    colour *= 1.0 - 0.3 * dot(off, off);
    colour *= 1.0 - finish.veil.y * smoothstep(finish.veil.x - 0.16 * size.y, finish.veil.x + 0.05 * size.y, pixel.x);
    colour *= 1.0 - 0.92 * smoothstep(finish.foot.x - 0.07 * size.y, finish.foot.x + 0.02 * size.y, pixel.y);
    let dither = (hash(pixel + finish.view.z) + hash(pixel * 1.7 + 11.0) - 1.0) / 255.0;
    textureStore(screen, vec2<i32>(id.xy), vec4<f32>(clamp(colour + dither, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0));
}
