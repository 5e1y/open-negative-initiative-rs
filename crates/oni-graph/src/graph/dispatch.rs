//! One stage on the GPU: its bind group layout and pipeline, cached by kind and arity, then the pass.

use super::*;

pub(super) fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

pub(super) fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

impl Graph {
    /// One stage over `output_rect`; `input` and each of `aux` is (texture, rectangle it covers, extent).
    pub(super) fn dispatch(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        stage: &dyn Stage,
        input: (&FloatTexture, Rect, Rect),
        aux: &[(&FloatTexture, Rect, Rect)],
        output: &FloatTexture,
        output_rect: Rect,
    ) {
        let (texture, input_rect, input_extent) = input;
        let frame = Frame {
            out_origin: [output_rect.x, output_rect.y],
            in_origin: [input_rect.x, input_rect.y],
            in_min: [input_extent.x, input_extent.y],
            in_max: [input_extent.right(), input_extent.bottom()],
            out_size: [output_rect.w, output_rect.h],
            pad: [0; 2],
        };
        let mut params = stage.params();
        // A uniform binding holds at least 16 bytes, in 16-byte steps.
        params.resize(params.len().div_ceil(16).max(1) * 16, 0);
        let frame = self.uniform(bytemuck::bytes_of(&frame));
        let params = self.uniform(&params);
        let mut frames = [AuxFrame::default(); MAX_AUX];
        for (f, (_, rect, extent)) in frames.iter_mut().zip(aux) {
            *f = AuxFrame { in_origin: [rect.x, rect.y], in_min: [extent.x, extent.y], in_max: [extent.right(), extent.bottom()], pad: [0; 2] };
        }
        let frames = (!aux.is_empty()).then(|| self.uniform(bytemuck::bytes_of(&frames)));
        let buffer = &self.uniforms.buffer.clone();
        let bound = |(offset, size): (u64, u64)| wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer, offset, size: wgpu::BufferSize::new(size) });
        let mut entries = vec![
            wgpu::BindGroupEntry { binding: 0, resource: bound(frame) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(texture.view()) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(output.view()) },
            wgpu::BindGroupEntry { binding: 3, resource: bound(params) },
        ];
        if let Some(frames) = frames {
            entries.push(wgpu::BindGroupEntry { binding: 4, resource: bound(frames) });
            for (k, (t, _, _)) in aux.iter().enumerate() {
                entries.push(wgpu::BindGroupEntry { binding: 5 + k as u32, resource: wgpu::BindingResource::TextureView(t.view()) });
            }
        }
        let pipeline = self.pipeline(stage, aux.len());
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(stage.name()),
            layout: &self.layouts[&aux.len()].group,
            entries: &entries,
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(stage.name()), timestamp_writes: None });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(output_rect.w.div_ceil(8), output_rect.h.div_ceil(8), 1);
        self.dispatches += 1;
    }

    /// `bytes` written at the ring's next aligned place; its offset and size there.
    fn uniform(&mut self, bytes: &[u8]) -> (u64, u64) {
        let u = &mut self.uniforms;
        let size = (bytes.len() as u64).div_ceil(16).max(1) * 16;
        if u.at + size > UNIFORM_RING {
            u.at = 0;
        }
        let at = u.at;
        let mut padded = bytes.to_vec();
        padded.resize(size as usize, 0);
        self.queue.write_buffer(&u.buffer, at, &padded);
        u.at = (at + size).div_ceil(u.align) * u.align;
        (at, size)
    }

    pub(super) fn layout(&mut self, arity: usize) -> &Layout {
        let device = &self.device;
        self.layouts.entry(arity).or_insert_with(|| {
            let mut entries = vec![
                uniform_entry(0),
                texture_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: crate::texture::FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                uniform_entry(3),
            ];
            if arity > 0 {
                entries.push(uniform_entry(4));
                entries.extend((0..arity).map(|k| texture_entry(5 + k as u32)));
            }
            let group = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("stage"), entries: &entries });
            let pipeline = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("stage"),
                bind_group_layouts: &[Some(&group)],
                immediate_size: 0,
            });
            Layout { group, pipeline }
        })
    }

    pub(super) fn pipeline(&mut self, stage: &dyn Stage, arity: usize) -> wgpu::ComputePipeline {
        let mut hasher = DefaultHasher::new();
        (stage.shader(), arity).hash(&mut hasher);
        let key = hasher.finish();
        if let Some(p) = self.pipelines.get(&key) {
            return p.clone();
        }
        let source = format!("{PRELUDE}\n{}\n{}", aux_prelude(arity), stage.shader());
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(stage.name()),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let layout = self.layout(arity).pipeline.clone();
        let layout = &layout;
        let pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(stage.name()),
            layout: Some(layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        self.pipelines.insert(key, pipeline.clone());
        pipeline
    }
}
