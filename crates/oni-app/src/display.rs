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

/// The kernel's uniform block: the matrix by rows, then width, height, seed and the dither's amplitude.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Params {
    pub rows: [[f32; 4]; 3],
    pub width: u32,
    pub height: u32,
    pub seed: u32,
    /// In quantisation steps; 0 turns the dither off.
    pub dither: f32,
}

impl Params {
    pub fn new(screen: Screen, width: u32, height: u32, seed: u32) -> Params {
        let m = screen.matrix();
        Params { rows: m.map(|r| [r[0], r[1], r[2], 0.0]), width, height, seed, dither: 1.0 }
    }
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
};

@group(0) @binding(0) var<uniform> k: Params;
@group(0) @binding(1) var<storage, read> source: array<vec4<f32>>;
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
    let p = source[id.y * k.width + id.x];
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

/// The kernel on a device: one dispatch per frame into a texture Slint can import.
pub struct Gpu {
    pipeline: wgpu::ComputePipeline,
}

impl Gpu {
    pub fn new(device: &wgpu::Device) -> Gpu {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("display"),
            source: wgpu::ShaderSource::Wgsl(KERNEL.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("display"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Gpu { pipeline }
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

    pub fn run(&self, device: &wgpu::Device, queue: &wgpu::Queue, pixels: &[[f32; 4]], k: &Params, target: &wgpu::Texture) {
        use wgpu::util::DeviceExt;
        let block = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("display params"),
            contents: bytemuck::bytes_of(k),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bytes: &[u8] = if pixels.is_empty() { &[0; 16] } else { bytemuck::cast_slice(pixels) };
        let source = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("linear"),
            contents: bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let view = target.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("display"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: block.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: source.as_entire_binding() },
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
        for screen in [Screen::Srgb, Screen::DisplayP3] {
            let k = Params::new(screen, w, h, 3);
            let target = Gpu::target(&device, w, h);
            gpu.run(&device, &queue, &pixels, &k, &target);
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
