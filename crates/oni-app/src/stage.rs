//! The welcome's stage: a wrapped gift in a black studio, drawn by the GPU into the texture the welcome shows.
//! `Frame` is one picture's whole description; `direction` writes it from the welcome's clock.

use bytemuck::{Pod, Zeroable};

pub mod direction;
mod ease;
pub use ease::glide;
pub mod unwrap;

#[cfg(test)]
mod tests;

/// One gift as the shader reads it; the fields' meaning is `scene.wgsl`'s.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Gift {
    pub place: [f32; 4],
    pub pose: [f32; 4],
    pub lid: [f32; 4],
    pub tie: [f32; 4],
    pub bow: [f32; 4],
    pub shape: [f32; 4],
}

/// One picture of the stage; the fields' meaning is `scene.wgsl`'s.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Frame {
    pub view: [f32; 4],
    pub eye: [f32; 4],
    pub aim: [f32; 4],
    pub light: [f32; 4],
    pub lens: [f32; 4],
    pub show: [f32; 4],
    pub gifts: [Gift; 4],
}

/// The shader that draws the stage in light, its parts in the order they name one another.
pub fn source() -> String {
    [include_str!("stage/scene.wgsl"), include_str!("stage/light.wgsl"), include_str!("stage/picture.wgsl")].join("\n")
}

/// The shader that finishes the light for the screen.
pub const FINISH: &str = include_str!("stage/finish.wgsl");

/// The finishing pass's block; the fields' meaning is `finish.wgsl`'s.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Finish {
    view: [f32; 4],
    veil: [f32; 4],
    foot: [f32; 4],
}

/// The pictures between the stage's light and the screen, for one size of target.
struct Between {
    size: (u32, u32),
    light: wgpu::TextureView,
    /// The light with its edges drawn again, which the finishing reads.
    sharp: wgpu::TextureView,
    gathered: wgpu::TextureView,
    spread: wgpu::TextureView,
}

/// The stage's pipelines and the pictures between its light and the screen.
pub struct Stage {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    /// The stage's block twice: for the pass that draws it, then for the one that draws its edges again.
    blocks: [wgpu::Buffer; 2],
    finishing: wgpu::ComputePipeline,
    finish_layout: wgpu::BindGroupLayout,
    passes: [wgpu::Buffer; 4],
    sampler: wgpu::Sampler,
    between: Option<Between>,
}

fn piped(device: &wgpu::Device, label: &str, source: &str, entries: &[wgpu::BindGroupLayoutEntry]) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout) {
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(label), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    (pipeline, layout)
}

/// A picture of light the passes write and sample.
fn light(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("stage light"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&Default::default())
}

impl Stage {
    pub fn new(device: &wgpu::Device) -> Stage {
        Stage::with_source(device, &source(), FINISH)
    }

    /// The stage drawn by `source` and finished by `finish`, which a look's trial reads from its files.
    pub fn with_source(device: &wgpu::Device, source: &str, finish: &str) -> Stage {
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty, count: None };
        let block = wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None };
        let picture = wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false };
        let written = |format| wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format, view_dimension: wgpu::TextureViewDimension::D2 };
        let sampled = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
        let (float, bytes) = (wgpu::TextureFormat::Rgba16Float, wgpu::TextureFormat::Rgba8Unorm);
        let stage = [entry(0, block), entry(1, written(float)), entry(2, picture)];
        let (pipeline, layout) = piped(device, "stage", source, &stage);
        let finishing = [entry(0, block), entry(1, picture), entry(2, picture), entry(3, sampled), entry(4, written(float)), entry(5, written(bytes))];
        let (finishing, finish_layout) = piped(device, "stage finish", finish, &finishing);
        let usage = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
        let buffer = |size: usize| device.create_buffer(&wgpu::BufferDescriptor { label: Some("stage block"), size: size as u64, usage, mapped_at_creation: false });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        Stage {
            pipeline,
            layout,
            blocks: std::array::from_fn(|_| buffer(std::mem::size_of::<Frame>())),
            finishing,
            finish_layout,
            passes: std::array::from_fn(|_| buffer(std::mem::size_of::<Finish>())),
            sampler,
            between: None,
        }
    }

    /// The seconds this GPU takes over a million pixels of the opened huge gift, the stage's heaviest picture: the
    /// least of a few, each awaited, so a busy moment counts for nothing.
    pub fn pace(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> f32 {
        let (width, height) = (960, 600);
        let target = Stage::target(device, width, height);
        let frame = direction::frame(&direction::Cue::at(Some(2), Some(30.0)));
        let mut least = f32::MAX;
        for _ in 0..6 {
            let began = std::time::Instant::now();
            self.draw(device, queue, &frame, &target);
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
            least = least.min(began.elapsed().as_secs_f32());
        }
        self.between = None;
        least / (width * height) as f32 * 1e6
    }

    /// A texture the stage draws into and Slint samples.
    pub fn target(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        crate::display::Gpu::target(device, width, height)
    }

    /// `frame` drawn into `target`, at the target's own size: the stage in light, its glow, then the screen's picture.
    pub fn draw(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &Frame, target: &wgpu::Texture) {
        let (width, height) = (target.width(), target.height());
        let quarter = (width.div_ceil(4), height.div_ceil(4));
        if self.between.as_ref().is_none_or(|b| b.size != (width, height)) {
            let (whole, small) = (|| light(device, width, height), || light(device, quarter.0, quarter.1));
            self.between = Some(Between { size: (width, height), light: whole(), sharp: whole(), gathered: small(), spread: small() });
        }
        let Some(between) = &self.between else { return };
        let mut frame = *frame;
        (frame.view[0], frame.view[1]) = (width as f32, height as f32);
        for (pass, block) in self.blocks.iter().enumerate() {
            frame.show[2] = pass as f32;
            queue.write_buffer(block, 0, bytemuck::bytes_of(&frame));
        }
        // The words' veil begins where their column does; with no column, past the picture.
        let from = width as f32 - frame.lens[3] * height as f32 + if frame.lens[3] > 0.0 { 0.0 } else { height as f32 };
        // Gather, spread near, spread wide, finish: the two spreads make one soft halo without a tap showing.
        for (block, (pass, reach)) in self.passes.iter().zip([(0.0, 0.0), (1.0, 0.007), (1.0, 0.018), (2.0, 0.0)]) {
            let finish = Finish { view: [width as f32, height as f32, frame.view[2], pass], veil: [from, 0.8, 0.26, reach], foot: [height as f32 * (1.0 - frame.show[3]), 0.0, 0.0, 0.0] };
            queue.write_buffer(block, 0, bytemuck::bytes_of(&finish));
        }
        let view = target.create_view(&Default::default());
        fn seen(view: &wgpu::TextureView) -> wgpu::BindingResource<'_> {
            wgpu::BindingResource::TextureView(view)
        }
        // The stage is drawn into one picture of light, then its edges again into the other, which reads the first.
        let stage = |pass: usize, into: &wgpu::TextureView, from: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("stage"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.blocks[pass].as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: seen(into) },
                    wgpu::BindGroupEntry { binding: 2, resource: seen(from) },
                ],
            })
        };
        let stages = [stage(0, &between.light, &between.sharp), stage(1, &between.sharp, &between.light)];
        // Each pass reads one small picture and writes the other; the last reads the light and the halo.
        let finish = |pass: usize, glow: &wgpu::TextureView, small: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("stage finish"),
                layout: &self.finish_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.passes[pass].as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: seen(&between.sharp) },
                    wgpu::BindGroupEntry { binding: 2, resource: seen(glow) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    wgpu::BindGroupEntry { binding: 4, resource: seen(small) },
                    wgpu::BindGroupEntry { binding: 5, resource: seen(&view) },
                ],
            })
        };
        let (one, other) = (&between.gathered, &between.spread);
        let groups = [finish(0, other, one), finish(1, one, other), finish(2, other, one), finish(3, one, other)];
        let mut encoder = device.create_command_encoder(&Default::default());
        for group in &stages {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        for (i, group) in groups.iter().enumerate() {
            let (w, h) = if i < 3 { quarter } else { (width, height) };
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.finishing);
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        queue.submit([encoder.finish()]);
    }
}
