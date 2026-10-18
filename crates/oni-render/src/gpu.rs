//! Every kernel on whatever `wgpu` finds: Metal on macOS, DirectX 12 on Windows, Vulkan elsewhere.
//! Images travel as storage buffers of `vec4<f32>`, rows top first; one `pass` runs any kernel.

use std::collections::HashMap;
use std::sync::Mutex;

use wgpu::util::DeviceExt;

use crate::prelude::uniform;
use crate::{Image, Rect};

const PRELUDE: &str = include_str!("wgsl/prelude.wgsl");
pub(crate) const DETAIL_TAPS: &str = include_str!("wgsl/detail_taps.wgsl");
const WORKGROUP: u32 = 64;
const MAX_GROUPS: u32 = 65_535;

/// One kernel: the shared prelude, then `parts`, entry point `main`. Binding 0 is the uniform
/// block, then each buffer `pass` receives, in order.
#[derive(Clone, Copy)]
pub(crate) struct Shader {
    pub(crate) name: &'static str,
    pub(crate) parts: &'static [&'static str],
}

macro_rules! shader {
    ($name:literal) => {
        Shader { name: $name, parts: &[include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/wgsl/", $name, ".wgsl"))] }
    };
    ($name:literal, $lib:expr) => {
        Shader { name: $name, parts: &[$lib, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/wgsl/", $name, ".wgsl"))] }
    };
}
pub(crate) use shader;

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: Mutex<HashMap<&'static str, wgpu::ComputePipeline>>,
    /// The largest buffer one binding may hold on this adapter.
    pub max_binding: u64,
    pub adapter: String,
}

#[derive(Debug)]
pub enum GpuError {
    NoAdapter(String),
    NoDevice(String),
}

pub(crate) fn head(count: usize, a: usize, b: usize, c: usize) -> [u32; 4] {
    let n = |v: usize| u32::try_from(v).expect("dimension past u32");
    [n(count), n(a), n(b), n(c)]
}

fn same_shape(a: &Image, b: &Image, what: &str) {
    assert!(a.width == b.width && a.height == b.height, "{what}: {}x{} against {}x{}", a.width, a.height, b.width, b.height);
}

/// A uniform row of whole numbers, laid bit for bit where the shader reads a `vec4<u32>`.
pub(crate) fn whole(v: [usize; 4]) -> [f32; 4] {
    v.map(|x| f32::from_bits(u32::try_from(x).expect("dimension past u32")))
}

impl Gpu {
    pub fn new() -> Result<Self, GpuError> {
        pollster::block_on(Self::open())
    }

    async fn open() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .map_err(|e| GpuError::NoAdapter(e.to_string()))?;
        let info = adapter.get_info();
        // The adapter's own limits: the defaults bind 128 MiB, a third of one 24 MP frame in rgba32float.
        let descriptor = wgpu::DeviceDescriptor { required_limits: adapter.limits(), ..Default::default() };
        let (device, queue) = adapter.request_device(&descriptor).await.map_err(|e| GpuError::NoDevice(e.to_string()))?;
        Ok(Self::from_device(device, queue, format!("{} ({:?})", info.name, info.backend)))
    }

    /// The kernels on a device another crate already opened, so one GPU serves the whole app.
    /// Bands follow the binding limit the caller requested with that device.
    pub fn from_device(device: wgpu::Device, queue: wgpu::Queue, adapter: String) -> Self {
        let limits = device.limits();
        let max_binding = limits.max_storage_buffer_binding_size.min(limits.max_buffer_size);
        Self { device, queue, pipelines: Mutex::new(HashMap::new()), max_binding, adapter }
    }

    // MARK: - Plumbing

    pub(crate) fn pipeline(&self, shader: Shader) -> wgpu::ComputePipeline {
        let mut cache = self.pipelines.lock().expect("pipeline cache");
        cache
            .entry(shader.name)
            .or_insert_with(|| {
                let source: String = std::iter::once(PRELUDE).chain(shader.parts.iter().copied()).collect();
                let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(shader.name),
                    source: wgpu::ShaderSource::Wgsl(source.into()),
                });
                self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(shader.name),
                    layout: None,
                    module: &module,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                })
            })
            .clone()
    }

    /// Refuses, by name, a buffer one binding cannot hold: such an image is tiled upstream.
    pub(crate) fn fits(&self, what: &str, texels: usize) {
        let bytes = texels as u64 * 16;
        assert!(bytes <= self.max_binding, "{what}: {bytes} bytes exceed this adapter's binding of {}; tile it", self.max_binding);
    }

    /// A buffer holding `pixels`; never empty, since a binding cannot be.
    pub(crate) fn upload(&self, pixels: &[[f32; 4]]) -> wgpu::Buffer {
        self.fits("upload", pixels.len());
        let bytes: &[u8] = if pixels.is_empty() { &[0; 16] } else { bytemuck::cast_slice(pixels) };
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        })
    }

    /// A zeroed buffer of `len` texels of 16 bytes.
    pub(crate) fn blank(&self, len: usize) -> wgpu::Buffer {
        self.fits("buffer", len);
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (len.max(1) * 16) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    /// Runs `shader` over `invocations`, the uniform block at binding 0 and `buffers` from binding 1.
    pub(crate) fn pass(&self, shader: Shader, uniform: &[u8], buffers: &[&wgpu::Buffer], invocations: usize) {
        let pipeline = self.pipeline(shader);
        let block = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params"),
            contents: uniform,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let mut entries = vec![wgpu::BindGroupEntry { binding: 0, resource: block.as_entire_binding() }];
        entries.extend(
            buffers.iter().enumerate().map(|(i, b)| wgpu::BindGroupEntry { binding: i as u32 + 1, resource: b.as_entire_binding() }),
        );
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(shader.name),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let total = (invocations as u64).div_ceil(WORKGROUP as u64).max(1);
        let x = total.min(MAX_GROUPS as u64);
        let y = total.div_ceil(x);
        assert!(y <= MAX_GROUPS as u64, "{invocations} invocations exceed one dispatch");
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(x as u32, y as u32, 1);
        }
        self.queue.submit([encoder.finish()]);
    }

    /// The first `len` texels of `buffer`, read back.
    pub(crate) fn download(&self, buffer: &wgpu::Buffer, len: usize) -> Vec<[f32; 4]> {
        let bytes = (len * 16) as u64;
        if bytes == 0 {
            return Vec::new();
        }
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(buffer, 0, &readback, 0, bytes);
        self.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("readback map"));
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let out = bytemuck::cast_slice(&slice.get_mapped_range().expect("mapped range")).to_vec();
        readback.unmap();
        out
    }

    /// One pass from images to a fresh output of `len` texels, read back.
    pub(crate) fn run(&self, shader: Shader, uniform: &[u8], inputs: &[&[[f32; 4]]], len: usize, invocations: usize) -> Vec<[f32; 4]> {
        if len == 0 {
            return Vec::new();
        }
        let uploaded: Vec<_> = inputs.iter().map(|p| self.upload(p)).collect();
        let output = self.blank(len);
        let mut buffers: Vec<&wgpu::Buffer> = uploaded.iter().collect();
        buffers.push(&output);
        self.pass(shader, uniform, &buffers, invocations);
        self.download(&output, len)
    }

    /// A per-pixel kernel on images of one shape, the output taking the first one's.
    pub(crate) fn per_pixel(
        &self,
        shader: Shader,
        rows: impl Fn(&Image) -> Vec<[f32; 4]>,
        inputs: &[&Image],
        tables: &[&[[f32; 4]]],
        aux: usize,
    ) -> Image {
        self.banded(shader, rows, inputs[0], inputs, tables, aux)
    }

    /// Runs over `shape` in bands of whole rows each binding can hold; `rows` builds the uniform
    /// rows for a band, which carries its own origin and height. `tables` bind whole after the images.
    pub(crate) fn banded(
        &self,
        shader: Shader,
        rows: impl Fn(&Image) -> Vec<[f32; 4]>,
        shape: &Image,
        inputs: &[&Image],
        tables: &[&[[f32; 4]]],
        aux: usize,
    ) -> Image {
        for other in inputs {
            same_shape(shape, other, shader.name);
        }
        let (w, h) = (shape.width, shape.height);
        if w * h == 0 {
            return shape.with_pixels(Vec::new());
        }
        let band = ((self.max_binding / 16) as usize / w).clamp(1, h);
        let mut out = Vec::with_capacity(w * h);
        for y0 in (0..h).step_by(band) {
            let y1 = (y0 + band).min(h);
            let n = (y1 - y0) * w;
            let origin = [shape.origin[0], shape.origin[1] + (h - y1) as f32];
            let part = Image { width: w, height: y1 - y0, origin, pixels: Vec::new() };
            let block = uniform(head(n, w, y1 - y0, aux), &rows(&part));
            let mut buffers: Vec<&[[f32; 4]]> = inputs.iter().map(|i| &i.pixels[y0 * w..y1 * w]).collect();
            buffers.extend_from_slice(tables);
            out.extend(self.run(shader, &block, &buffers, n, n));
        }
        shape.with_pixels(out)
    }

    // MARK: - pipeline.metal

    // MARK: - curves.metal

    // MARK: - chroma.metal

    // MARK: - gamut.metal

    // MARK: - rgbPasses.metal

    // MARK: - scanner.metal

    // MARK: - correctionSearch.metal

    // MARK: - Core Image built-ins

}

pub(crate) fn check_rect(image: &Image, rect: Rect) {
    assert!(
        rect.width > 0 && rect.height > 0 && rect.x + rect.width <= image.width && rect.y + rect.height <= image.height,
        "{rect:?} outside {}x{}",
        image.width,
        image.height
    );
}
