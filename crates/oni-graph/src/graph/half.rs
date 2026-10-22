//! Renders kept in half floats: a reduction of a decode is evaluated in full floats, then packed.

use super::*;

const SHADER: &str = "
struct Offset { x: i32, y: i32, pad: vec2<i32> }
@group(0) @binding(0) var<uniform> offset: Offset;
@group(0) @binding(1) var input: texture_2d<f32>;
@group(0) @binding(2) var output: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(output);
    if (id.x >= size.x || id.y >= size.y) { return; }
    let p = vec2<i32>(id.xy);
    textureStore(output, p, textureLoad(input, p + vec2<i32>(offset.x, offset.y), 0));
}
";

pub(super) struct Packer {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
}

impl Packer {
    fn new(device: &wgpu::Device) -> Packer {
        let storage = wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::StorageTexture {
                access: wgpu::StorageTextureAccess::WriteOnly,
                format: Precision::Half.format(),
                view_dimension: wgpu::TextureViewDimension::D2,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pack"),
            entries: &[dispatch::uniform_entry(0), dispatch::texture_entry(1), storage],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("pack"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("pack"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("pack"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Packer { layout, pipeline }
    }
}

impl Graph {
    /// `materialise` held in half floats: what a decode's reductions keep, at half the memory. A render
    /// past one texture stays in full floats.
    pub fn materialise_half(&mut self, image: &Image) -> Result<Source, Error> {
        let region = image.extent();
        let image = image.readable();
        let plan = Plan::new(&image, region, None)?;
        if !self.fits(&plan) {
            drop(plan);
            return self.materialise(&image);
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let (texture, rect) = self.evaluate(&mut encoder, plan, None, false);
        let half = self.pool.create_owned(region.w, region.h, Precision::Half);
        let offset = self.uniform(bytemuck::bytes_of(&[region.x - rect.x, region.y - rect.y, 0, 0]));
        let packer = self.packer.get_or_insert_with(|| Packer::new(&self.device));
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pack"),
            layout: &packer.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &self.uniforms.buffer, offset: offset.0, size: wgpu::BufferSize::new(offset.1) }),
                },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(texture.view()) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(half.view()) },
            ],
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("pack"), timestamp_writes: None });
            pass.set_pipeline(&packer.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(region.w.div_ceil(8), region.h.div_ceil(8), 1);
        }
        self.submit(encoder);
        self.pool.release(texture);
        self.ended();
        let id = SOURCE_IDS.fetch_add(1, Ordering::Relaxed);
        Ok(Source(Arc::new(SourceInner { id, extent: region, tiles: vec![(region, half)] })))
    }
}

impl Image {
    /// The image itself, or past a source in half floats a pass that reads it into full floats, which
    /// a readback or a copy into a full-float texture needs.
    pub(super) fn readable(&self) -> Image {
        match self.stages.is_empty() && self.source.precision() == Precision::Half {
            true => self.clone().then(crate::stages::Crop(self.extent())),
            false => self.clone(),
        }
    }
}
