//! The fused kernel on whatever `wgpu` finds: Metal on macOS, DirectX 12 on Windows, Vulkan elsewhere.

use wgpu::util::DeviceExt;

use crate::Stage47;

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    pub adapter: String,
}

#[derive(Debug)]
pub enum GpuError {
    NoAdapter(String),
    NoDevice(String),
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
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| GpuError::NoDevice(e.to_string()))?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("stage47"),
            source: wgpu::ShaderSource::Wgsl(include_str!("stage47.wgsl").into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("stage47"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Ok(Self { device, queue, pipeline, adapter: format!("{} ({:?})", info.name, info.backend) })
    }

    /// Runs the kernel on `pixels` and reads the result back, in rgba32float throughout.
    pub fn stage47(&self, pixels: &[[f32; 4]], k: &Stage47) -> Vec<[f32; 4]> {
        let count = pixels.len() as u32;
        let bytes = std::mem::size_of_val(pixels) as u64;
        let uniform = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params"),
            contents: bytemuck::bytes_of(&k.packed(count)),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let input = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("in"),
            contents: bytemuck::cast_slice(pixels),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let output = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("out"),
            size: bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = self.pipeline.get_bind_group_layout(0);
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: input.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: output.as_entire_binding() },
            ],
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(count.div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, bytes);
        self.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("readback map"));
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let out = bytemuck::cast_slice(&slice.get_mapped_range().expect("mapped range")).to_vec();
        readback.unmap();
        out
    }
}
