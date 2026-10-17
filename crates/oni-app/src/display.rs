//! The one place 8 bits appear: linear Rec. 2020 to the screen's space, encoded, dithered and
//! quantised into an Rgba8Unorm texture the canvas imports. `encode` is the kernel's CPU reference.

use bytemuck::{Pod, Zeroable};

/// The screen the canvas is proofed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Srgb,
    DisplayP3,
}

type M3 = [[f64; 3]; 3];

/// CIE xy of the red, green and blue primaries; every space here sits on D65.
const REC2020: [[f64; 2]; 3] = [[0.708, 0.292], [0.170, 0.797], [0.131, 0.046]];
const SRGB: [[f64; 2]; 3] = [[0.64, 0.33], [0.30, 0.60], [0.15, 0.06]];
const P3: [[f64; 2]; 3] = [[0.680, 0.320], [0.265, 0.690], [0.150, 0.060]];
const D65: [f64; 2] = [0.3127, 0.3290];

fn mul(a: &M3, b: &M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn inverse(m: &M3) -> M3 {
    let c = |r: usize, s: usize| {
        let (r0, r1) = ((r + 1) % 3, (r + 2) % 3);
        let (s0, s1) = ((s + 1) % 3, (s + 2) % 3);
        m[r0][s0] * m[r1][s1] - m[r0][s1] * m[r1][s0]
    };
    let det: f64 = (0..3).map(|j| m[0][j] * c(0, j)).sum();
    std::array::from_fn(|i| std::array::from_fn(|j| c(j, i) / det))
}

/// The RGB to XYZ matrix of a space, its white mapping to Y = 1.
fn to_xyz(primaries: &[[f64; 2]; 3]) -> M3 {
    let xyz = |p: [f64; 2]| [p[0] / p[1], 1.0, (1.0 - p[0] - p[1]) / p[1]];
    let columns = primaries.map(xyz);
    let m: M3 = std::array::from_fn(|i| std::array::from_fn(|j| columns[j][i]));
    let white = xyz(D65);
    let inv = inverse(&m);
    let s: [f64; 3] = std::array::from_fn(|i| (0..3).map(|k| inv[i][k] * white[k]).sum());
    std::array::from_fn(|i| std::array::from_fn(|j| m[i][j] * s[j]))
}

impl Screen {
    /// Linear Rec. 2020 to this screen's linear primaries.
    pub fn matrix(self) -> [[f32; 3]; 3] {
        let target = match self {
            Screen::Srgb => &SRGB,
            Screen::DisplayP3 => &P3,
        };
        mul(&inverse(&to_xyz(target)), &to_xyz(&REC2020)).map(|r| r.map(|v| v as f32))
    }
}

/// The kernel's uniform block: the matrix by rows, width, height, seed and the dither's amplitude,
/// then `ScreenGamut`'s zebra: the screen's extended range, its tint, where a texel lands on the canvas.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Params {
    pub rows: [[f32; 4]; 3],
    pub width: u32,
    pub height: u32,
    pub seed: u32,
    /// In quantisation steps; 0 turns the dither off.
    pub dither: f32,
    pub low: [f32; 4],
    pub high: [f32; 4],
    /// Working light, as a pixel of the picture is.
    pub tint: [f32; 4],
    /// The texture's corner on the canvas, the canvas pixels per texel, the canvas's height.
    pub origin: [f32; 2],
    pub stretch: f32,
    pub canvas_height: f32,
    pub phase: f32,
    pub period: f32,
    pub duty: f32,
    /// 1 stripes what the screen cannot show, 0 leaves the picture alone.
    pub zebra: f32,
}

/// Where the zebra is laid: the texture's place on the canvas, in drawable pixels, and the clock.
#[derive(Clone, Copy, Debug)]
pub struct Zebra {
    pub origin: (f32, f32),
    pub stretch: f32,
    pub canvas_height: f32,
    pub phase: f32,
}

impl Params {
    pub fn new(screen: Screen, width: u32, height: u32, seed: u32) -> Params {
        let m = screen.matrix();
        Params {
            rows: m.map(|r| [r[0], r[1], r[2], 0.0]),
            width,
            height,
            seed,
            dither: 1.0,
            low: [0.0; 4],
            high: [1.0; 4],
            tint: [0.0; 4],
            origin: [0.0; 2],
            stretch: 1.0,
            canvas_height: height as f32,
            phase: 0.0,
            period: oni_colour::gamut::PERIOD,
            duty: oni_colour::gamut::DUTY,
            zebra: 0.0,
        }
    }

    /// With the zebra on: the range is the screen's, mirrored under zero and continued past one by
    /// `TOLERANCE`, as the system extends a display's space; sRGB and Display P3 share the curve.
    pub fn striped(self, z: Zebra) -> Params {
        let tolerance = oni_colour::gamut::TOLERANCE;
        let decode = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
        let (low, high) = (-decode(tolerance), decode(1.0 + tolerance));
        Params {
            low: [low, low, low, 0.0],
            high: [high, high, high, 0.0],
            tint: oni_colour::gamut::tint(),
            origin: [z.origin.0, z.origin.1],
            stretch: z.stretch,
            canvas_height: z.canvas_height,
            phase: z.phase,
            zebra: 1.0,
            ..self
        }
    }
}

/// True where a texel falls on a stripe the screen cannot show: `gamut::stripe` on its canvas pixel, y up.
fn on_stripe(x: u32, y: u32, k: &Params) -> bool {
    let cx = k.origin[0] + (x as f32 + 0.5) * k.stretch;
    let cy = k.canvas_height - (k.origin[1] + (y as f32 + 0.5) * k.stretch);
    (cx + cy + k.phase).rem_euclid(k.period) < k.duty * k.period
}

fn beyond(p: [f32; 4], k: &Params) -> bool {
    (0..3).any(|c| {
        let r = k.rows[c];
        let shown = r[0] * p[0] + r[1] * p[1] + r[2] * p[2];
        shown < k.low[c] || shown > k.high[c]
    })
}

/// `pcg` from Jarzynski and Olano, the hash both sides share bit for bit.
fn pcg(v: u32) -> u32 {
    let state = v.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277_803_737);
    (word >> 22) ^ word
}

fn unit(v: u32) -> f32 {
    (v >> 8) as f32 / 16_777_216.0
}

/// The sRGB transfer function, which Display P3 shares.
fn transfer(v: f32) -> f32 {
    if v <= 0.003_130_8 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

/// One pixel as the kernel writes it: a triangular dither of one step, then the nearest of 256 levels.
pub fn encode_pixel(p: [f32; 4], x: u32, y: u32, k: &Params) -> [u8; 4] {
    let p = if k.zebra > 0.5 && beyond(p, k) && on_stripe(x, y, k) { k.tint } else { p };
    let h = pcg(x ^ pcg(y ^ pcg(k.seed)));
    let mut out = [0u8, 0, 0, 255];
    for c in 0..3 {
        let r = k.rows[c];
        let linear = (r[0] * p[0] + r[1] * p[1] + r[2] * p[2]).clamp(0.0, 1.0);
        let a = pcg(h.wrapping_add(2 * c as u32));
        let b = pcg(h.wrapping_add(2 * c as u32 + 1));
        let d = (unit(a) + unit(b) - 1.0) * k.dither;
        out[c] = (transfer(linear) * 255.0 + d).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// The whole image on the CPU, row-major from the top: the software renderer's path.
pub fn encode(pixels: &[[f32; 4]], k: &Params) -> Vec<u8> {
    let w = k.width.max(1);
    pixels.iter().enumerate().flat_map(|(i, p)| encode_pixel(*p, i as u32 % w, i as u32 / w, k)).collect()
}

pub const KERNEL: &str = r#"
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
        let linear = clamp(r.x * p.x + r.y * p.y + r.z * p.z, 0.0, 1.0);
        let d = (unit(pcg(h + 2u * c)) + unit(pcg(h + 2u * c + 1u)) - 1.0) * k.dither;
        // Quantised here, so the texture's own conversion only stores an exact level.
        out[c] = clamp(round(transfer(linear) * 255.0 + d), 0.0, 255.0) / 255.0;
    }
    textureStore(canvas, vec2<u32>(id.x, id.y), out);
}
"#;

/// Counts the shown levels of every second row and column, the sample the CPU histogram reads.
pub const HISTOGRAM_KERNEL: &str = r#"
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
"#;

const BINS: u64 = 256 * 3 * 4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mapping {
    Idle,
    Waiting,
    Mapped,
}

/// The kernels on a device: one dispatch per frame into a texture Slint can import, and on its own
/// pace the histogram of what it wrote, read back without waiting.
pub struct Gpu {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    histogram: wgpu::ComputePipeline,
    counts: wgpu::Buffer,
    staging: wgpu::Buffer,
    mapping: std::sync::Arc<std::sync::Mutex<Mapping>>,
}

impl Gpu {
    pub fn new(device: &wgpu::Device) -> Gpu {
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty, count: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("display"),
            entries: &[
                entry(0, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }),
                // rgba32float is not filterable without a feature: the kernel only loads texels.
                entry(1, wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                }),
                entry(2, wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    view_dimension: wgpu::TextureViewDimension::D2,
                }),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("display"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let compute = |label, source: &str, layout: Option<&wgpu::PipelineLayout>| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout,
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let pipeline = compute("display", KERNEL, Some(&pipeline_layout));
        let histogram = compute("histogram", HISTOGRAM_KERNEL, None);
        let buffer = |label, usage| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: BINS, usage, mapped_at_creation: false });
        let counts = buffer("histogram counts", wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST);
        let staging = buffer("histogram readback", wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
        Gpu { pipeline, layout, histogram, counts, staging, mapping: std::sync::Arc::new(std::sync::Mutex::new(Mapping::Idle)) }
    }

    /// A texture the kernel writes and Slint samples: `Image::try_from` takes only this format and usage.
    pub fn target(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("canvas"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    /// Encodes `source`, an rgba32float texture the size of `target`, into `target`.
    pub fn run(&self, device: &wgpu::Device, queue: &wgpu::Queue, source: &wgpu::TextureView, k: &Params, target: &wgpu::Texture) {
        use wgpu::util::DeviceExt;
        let block = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("display params"),
            contents: bytemuck::bytes_of(k),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let view = target.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("display"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: block.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(source) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&view) },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(k.width.div_ceil(8), k.height.div_ceil(8), 1);
        }
        queue.submit([encoder.finish()]);
    }

    /// Counts the levels `target` holds, unless the last count is still being read; says whether it did.
    pub fn count(&self, device: &wgpu::Device, queue: &wgpu::Queue, target: &wgpu::Texture) -> bool {
        {
            let mut m = self.mapping.lock().expect("histogram mapping");
            if *m != Mapping::Idle {
                return false;
            }
            *m = Mapping::Waiting;
        }
        let view = target.create_view(&Default::default());
        let counting = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("histogram"),
            layout: &self.histogram.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: self.counts.as_entire_binding() },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.clear_buffer(&self.counts, 0, None);
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.histogram);
            pass.set_bind_group(0, &counting, &[]);
            pass.dispatch_workgroups(target.width().div_ceil(2).div_ceil(8), target.height().div_ceil(2).div_ceil(8), 1);
        }
        encoder.copy_buffer_to_buffer(&self.counts, 0, &self.staging, 0, BINS);
        queue.submit([encoder.finish()]);
        let mapping = self.mapping.clone();
        self.staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            *mapping.lock().expect("histogram mapping") = if r.is_ok() { Mapping::Mapped } else { Mapping::Idle };
        });
        true
    }

    /// The last count once its readback has landed, which a `device.poll` delivers; none before.
    pub fn histogram(&self) -> Option<crate::render::Histogram> {
        let mut m = self.mapping.lock().expect("histogram mapping");
        if *m != Mapping::Mapped {
            return None;
        }
        let bins = {
            let range = self.staging.slice(..).get_mapped_range().ok()?;
            bytemuck::cast_slice::<u8, [u32; 3]>(&range).to_vec()
        };
        self.staging.unmap();
        *m = Mapping::Idle;
        Some(crate::render::Histogram { bins })
    }
}

/// `pixels` laid in an rgba32float texture the kernel reads, the software path's and the checks' input.
pub fn upload(device: &wgpu::Device, queue: &wgpu::Queue, pixels: &[[f32; 4]], width: u32, height: u32) -> wgpu::Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("linear"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        bytemuck::cast_slice(pixels),
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 16), rows_per_image: None },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    texture
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_matrices_keep_white_and_match_the_published_ones() {
        for screen in [Screen::Srgb, Screen::DisplayP3] {
            let m = screen.matrix();
            for r in m {
                assert!((r.iter().sum::<f32>() - 1.0).abs() < 1e-5, "{screen:?} moves white");
            }
        }
        // BT.2087's Rec. 2020 to Rec. 709 first row.
        let m = Screen::Srgb.matrix();
        assert!((m[0][0] - 1.6605).abs() < 1e-3 && (m[0][1] + 0.5876).abs() < 1e-3);
        // Twin: P3 is a wider target, so its red row pulls less than sRGB's.
        assert!(Screen::DisplayP3.matrix()[0][0] < m[0][0] - 0.2);
    }

    #[test]
    fn the_dither_breaks_a_ramp_into_noise_around_the_right_level() {
        let (w, h) = (256u32, 64u32);
        let grey: Vec<[f32; 4]> = (0..w * h).map(|_| [0.18, 0.18, 0.18, 1.0]).collect();
        let k = Params::new(Screen::Srgb, w, h, 7);
        let out = encode(&grey, &k);
        let levels: Vec<u8> = out.chunks(4).map(|p| p[0]).collect();
        let mean = levels.iter().map(|&v| v as f64).sum::<f64>() / levels.len() as f64;
        let exact = transfer(0.18) as f64 * 255.0;
        assert!((mean - exact).abs() < 0.05, "mean {mean} against {exact}");
        assert!(levels.iter().all(|&v| (v as f64 - exact).abs() < 2.0), "one step of noise at most either side");
        // Twin: without dither every pixel lands on one level, which is the banding the noise breaks.
        let flat = encode(&grey, &Params { dither: 0.0, ..k });
        let first = flat[0];
        assert!(flat.chunks(4).all(|p| p[0] == first));
        assert!(levels.iter().any(|&v| v != first));
    }

    /// The kernel on the first adapter wgpu finds, lavapipe included; skipped where there is none.
    #[test]
    fn the_kernel_writes_what_its_cpu_reference_writes() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            eprintln!("no adapter: the display kernel is not exercised");
            return;
        };
        let Ok((device, queue)) = pollster::block_on(adapter.request_device(&Default::default())) else { return };
        let (w, h) = (64u32, 16u32);
        let pixels: Vec<[f32; 4]> = (0..w * h)
            .map(|i| {
                let t = (i % w) as f32 / (w - 1) as f32;
                [t * 1.2 - 0.1, t * t, 1.0 - t, 1.0]
            })
            .collect();
        let gpu = Gpu::new(&device);
        let source = upload(&device, &queue, &pixels, w, h).create_view(&Default::default());
        for screen in [Screen::Srgb, Screen::DisplayP3] {
            let k = Params::new(screen, w, h, 3);
            let target = Gpu::target(&device, w, h);
            gpu.run(&device, &queue, &source, &k, &target);
            let got = read_back(&device, &queue, &target, w, h);
            let want = encode(&pixels, &k);
            let worst = got.iter().zip(&want).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            assert!(worst <= 1, "{screen:?}: {worst} levels apart");
            // Twin: the reference with the other screen's matrix does not match the kernel.
            let other = if screen == Screen::Srgb { Screen::DisplayP3 } else { Screen::Srgb };
            let wrong = encode(&pixels, &Params::new(other, w, h, 3));
            assert!(got.iter().zip(&wrong).any(|(a, b)| a.abs_diff(*b) > 2));
        }
    }

    /// The zebra on the GPU against its CPU reference, on colours sRGB cannot hold and one it can.
    #[test]
    fn the_zebra_stripes_what_the_screen_cannot_show_as_its_reference_does() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            eprintln!("no adapter: the zebra is not exercised");
            return;
        };
        let Ok((device, queue)) = pollster::block_on(adapter.request_device(&Default::default())) else { return };
        let (w, h) = (48u32, 20u32);
        // The left half a saturated Rec. 2020 green sRGB cannot show, the right half a grey it can.
        let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| if i % w < w / 2 { [0.05, 0.6, 0.05, 1.0] } else { [0.3, 0.3, 0.3, 1.0] }).collect();
        let gpu = Gpu::new(&device);
        let source = upload(&device, &queue, &pixels, w, h).create_view(&Default::default());
        let zebra = Zebra { origin: (10.0, 4.0), stretch: 2.0, canvas_height: 60.0, phase: 3.0 };
        let k = Params::new(Screen::Srgb, w, h, 3).striped(zebra);
        let target = Gpu::target(&device, w, h);
        gpu.run(&device, &queue, &source, &k, &target);
        let got = read_back(&device, &queue, &target, w, h);
        let want = encode(&pixels, &k);
        assert!(got.iter().zip(&want).all(|(a, b)| a.abs_diff(*b) <= 1));
        let plain = encode(&pixels, &Params::new(Screen::Srgb, w, h, 3));
        let changed = |half: std::ops::Range<u32>| (0..h).flat_map(|y| half.clone().map(move |x| (y * w + x) as usize * 4)).filter(|&i| want[i..i + 3] != plain[i..i + 3]).count();
        assert!(changed(0..w / 2) > 0, "the green the screen cannot show is striped");
        assert_eq!(changed(w / 2..w), 0, "the grey it can show is left alone");
        // Twin: another phase moves the stripes, so a kernel ignoring the clock would be seen.
        let moved = encode(&pixels, &Params::new(Screen::Srgb, w, h, 3).striped(Zebra { phase: 7.5, ..zebra }));
        assert_ne!(moved, want);
    }

    /// The GPU count against `Histogram::of` on the same shown bytes, sampled on every second row and column.
    #[test]
    fn the_histogram_kernel_counts_what_the_cpu_counts() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            eprintln!("no adapter: the histogram kernel is not exercised");
            return;
        };
        let Ok((device, queue)) = pollster::block_on(adapter.request_device(&Default::default())) else { return };
        let (w, h) = (37u32, 23u32);
        let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| [(i % w) as f32 / w as f32, (i / w) as f32 / h as f32, 0.4, 1.0]).collect();
        let gpu = Gpu::new(&device);
        let source = upload(&device, &queue, &pixels, w, h).create_view(&Default::default());
        let target = Gpu::target(&device, w, h);
        gpu.run(&device, &queue, &source, &Params::new(Screen::Srgb, w, h, 9), &target);
        assert!(gpu.count(&device, &queue, &target));
        assert!(!gpu.count(&device, &queue, &target), "a second count waits for the first to be read");
        device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let counted = gpu.histogram().expect("the count landed");
        let shown = read_back(&device, &queue, &target, w, h);
        let sampled: Vec<u8> = (0..h as usize)
            .step_by(2)
            .flat_map(|y| (0..w as usize).step_by(2).map(move |x| (y * w as usize + x) * 4))
            .flat_map(|i| shown[i..i + 4].to_vec())
            .collect();
        assert_eq!(counted, crate::render::Histogram::of(&sampled));
        // Twin: every pixel counted is another histogram, so a kernel that skips no row would be seen.
        assert_ne!(counted, crate::render::Histogram::of(&shown));
    }

    fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, w: u32, h: u32) -> Vec<u8> {
        let row = (w * 4).div_ceil(256) * 256;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let mapped = slice.get_mapped_range().expect("mapped range");
        (0..h as usize).flat_map(|y| mapped[y * row as usize..y * row as usize + w as usize * 4].to_vec()).collect()
    }
}
