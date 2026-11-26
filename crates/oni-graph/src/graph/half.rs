//! Renders kept in half floats: a reduction of a decode is evaluated in full floats, then packed.

use super::*;

const SHADER: &str = "
struct Band { read: vec2<i32>, write: vec2<i32>, size: vec2<i32>, pad: vec2<i32> }
@group(0) @binding(0) var<uniform> band: Band;
@group(0) @binding(1) var input: texture_2d<f32>;
@group(0) @binding(2) var output: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let p = vec2<i32>(id.xy);
    if (p.x >= band.size.x || p.y >= band.size.y) { return; }
    textureStore(output, p + band.write, textureLoad(input, p + band.read, 0));
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
    /// `materialise` held in half floats: what a decode's reductions keep, at half the memory. It is evaluated in bands
    /// of at most `BAND_PIXELS`, so a full frame's chain holds a band's textures, not the frame's; past one texture it
    /// stays in full floats.
    pub fn materialise_half(&mut self, image: &Image) -> Result<Source, Error> {
        self.materialise_half_cached(image, None)
    }

    /// `materialise_half` starting from what `caches` hold, one a band, and leaving in each what it keeps: a chain made
    /// again with a late stage moved reruns only what follows it.
    pub fn materialise_half_cached(&mut self, image: &Image, mut caches: Option<&mut Vec<Cache>>) -> Result<Source, Error> {
        let region = image.extent();
        let image = image.readable();
        let rows = ((BAND_PIXELS / u64::from(region.w.max(1))) as u32).clamp(1, region.h.max(1));
        let bands: Vec<Rect> = (0..region.h.div_ceil(rows)).map(|i| Rect::new(region.x, region.y + (i * rows) as i32, region.w, rows.min(region.h - i * rows))).collect();
        if let Some(c) = caches.as_deref_mut() {
            c.resize_with(bands.len(), Cache::frontier);
        }
        let mut plans = Vec::with_capacity(bands.len());
        for (i, band) in bands.iter().enumerate() {
            let plan = Plan::new(&image, *band, caches.as_deref().map(|c| &c[i]))?;
            if !self.fits(&plan) {
                drop(plans);
                drop(plan);
                return self.materialise(&image);
            }
            plans.push(plan);
        }
        let half = self.pool.create_owned(region.w, region.h, Precision::Half);
        for (i, (band, plan)) in bands.into_iter().zip(plans).enumerate() {
            let mut kept = caches.as_deref_mut().map(|c| &mut c[i]);
            if let Some(c) = kept.as_deref_mut() {
                c.begin(&plan.nodes);
            }
            let mut encoder = self.device.create_command_encoder(&Default::default());
            let (texture, rect) = self.evaluate(&mut encoder, plan, kept.as_deref_mut(), false);
            let at = [band.x - rect.x, band.y - rect.y, band.x - region.x, band.y - region.y, band.w as i32, band.h as i32, 0, 0];
            let uniform = self.uniform(bytemuck::bytes_of(&at));
            let packer = self.packer.get_or_insert_with(|| Packer::new(&self.device));
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("pack"),
                layout: &packer.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &self.uniforms.buffer, offset: uniform.0, size: wgpu::BufferSize::new(uniform.1) }),
                    },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(texture.view()) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(half.view()) },
                ],
            });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("pack"), timestamp_writes: None });
                pass.set_pipeline(&packer.pipeline);
                pass.set_bind_group(0, &group, &[]);
                pass.dispatch_workgroups(band.w.div_ceil(8), band.h.div_ceil(8), 1);
            }
            self.submit(encoder);
            self.pool.release(texture);
            self.settle(kept);
            self.ended();
        }
        let id = SOURCE_IDS.fetch_add(1, Ordering::Relaxed);
        Ok(Source(Arc::new(SourceInner { id, extent: region, tiles: vec![(region, half)] })))
    }
}

/// The pixels a band of a half-float render holds at most: a 24 MP frame's chain in six bands, each texture about
/// 64 MB in full floats rather than 380.
const BAND_PIXELS: u64 = 4_000_000;

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
