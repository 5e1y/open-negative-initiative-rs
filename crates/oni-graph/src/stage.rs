//! What a stage is: a WGSL function of one pixel, the region it reads, and the extent it covers.

use crate::Rect;

/// A WGSL `fn pixel(p: vec2<i32>) -> vec4<f32>` reading its input through `load(q)`, parameters at
/// binding 3; the graph owns every texture, a stage only names the pixels it reads.
pub trait Stage: Send + Sync {
    /// Names the stage's region rule in the cache key: two stages sharing a shader may read differently.
    fn name(&self) -> &'static str;

    fn shader(&self) -> &'static str;

    /// The bytes behind `params`, laid out as WGSL's uniform rules want. Also the cache key.
    fn params(&self) -> Vec<u8> {
        Vec::new()
    }

    /// The extent this stage produces from an input of extent `input`.
    fn extent(&self, input: Rect) -> Rect {
        input
    }

    /// Every input pixel `pixel` may read for an output covering `output`; a lie shows at the region's edges.
    fn input_region(&self, output: Rect) -> Rect {
        output
    }

    /// Every pixel of auxiliary input `index` read for `output`, in that input's own coordinates.
    fn aux_region(&self, index: usize, output: Rect) -> Rect {
        let _ = index;
        output
    }
}

/// The most auxiliary inputs one stage reads beside its own.
pub const MAX_AUX: usize = 3;

/// Where each auxiliary input lies, mirrored by `AuxFrame` in `aux_prelude`.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct AuxFrame {
    pub in_origin: [i32; 2],
    pub in_min: [i32; 2],
    pub in_max: [i32; 2],
    /// The rectangle the texture holds, which may be smaller than the texture.
    pub in_size: [i32; 2],
}

/// The bindings and readers of `arity` auxiliary inputs: `src1` with `load1` and `inside1`, and so on.
pub(crate) fn aux_prelude(arity: usize) -> String {
    if arity == 0 {
        return String::new();
    }
    let mut out = format!(
        r#"
struct AuxFrame {{
    in_origin: vec2<i32>,
    in_min: vec2<i32>,
    in_max: vec2<i32>,
    in_size: vec2<i32>,
}}

struct AuxFrames {{ f: array<AuxFrame, {MAX_AUX}> }}

@group(0) @binding(4) var<uniform> aux_frames: AuxFrames;

fn load_from(t: texture_2d<f32>, f: AuxFrame, q: vec2<i32>) -> vec4<f32> {{
    let e = clamp(q, f.in_min, f.in_max - vec2<i32>(1)) - f.in_origin;
    let c = clamp(e, vec2<i32>(0), f.in_size - vec2<i32>(1));
    return textureLoad(t, c, 0);
}}
"#
    );
    for k in 1..=arity {
        out.push_str(&format!(
            r#"
@group(0) @binding({b}) var src{k}: texture_2d<f32>;

fn load{k}(q: vec2<i32>) -> vec4<f32> {{
    return load_from(src{k}, aux_frames.f[{i}], q);
}}

fn inside{k}(q: vec2<i32>) -> bool {{
    let f = aux_frames.f[{i}];
    return all(q >= f.in_min) && all(q < f.in_max);
}}
"#,
            b = 4 + k,
            i = k - 1
        ));
    }
    out
}

/// Header of every dispatch, mirrored by `Frame` in `PRELUDE`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Frame {
    pub out_origin: [i32; 2],
    pub in_origin: [i32; 2],
    pub in_min: [i32; 2],
    pub in_max: [i32; 2],
    pub out_size: [u32; 2],
    /// The rectangle the input texture holds, which may be smaller than the texture.
    pub in_size: [u32; 2],
    /// Where the output's corner lies in its texture: a sheet holds many outputs.
    pub out_at: [i32; 2],
    pub pad: [i32; 2],
}

/// Prepended to every stage's shader: bindings, the coordinate change and the entry point.
pub(crate) const PRELUDE: &str = r#"
struct Frame {
    out_origin: vec2<i32>,
    in_origin: vec2<i32>,
    in_min: vec2<i32>,
    in_max: vec2<i32>,
    out_size: vec2<u32>,
    in_size: vec2<u32>,
    out_at: vec2<i32>,
    pad: vec2<i32>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var src: texture_2d<f32>;
@group(0) @binding(2) var dst: texture_storage_2d<rgba32float, write>;

fn inside(q: vec2<i32>) -> bool {
    return all(q >= frame.in_min) && all(q < frame.in_max);
}

// Clamped to the input's extent, then to what its texture holds, so a stage that under-declares reads defined values.
fn load(q: vec2<i32>) -> vec4<f32> {
    let e = clamp(q, frame.in_min, frame.in_max - vec2<i32>(1)) - frame.in_origin;
    let t = clamp(e, vec2<i32>(0), vec2<i32>(frame.in_size) - vec2<i32>(1));
    return textureLoad(src, t, 0);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= frame.out_size)) {
        return;
    }
    textureStore(dst, frame.out_at + vec2<i32>(id.xy), pixel(frame.out_origin + vec2<i32>(id.xy)));
}
"#;
