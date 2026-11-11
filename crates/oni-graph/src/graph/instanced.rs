//! One stage dispatched once for many instances, each with its own frame, parameters and places in the
//! textures they share: a pass costs the GPU about as much as it costs a small instance's pixels.

use super::*;

/// Where an image lies in a texture others share: the rectangle held, its corner there, and its extent.
#[derive(Clone, Copy)]
pub(super) struct Cell {
    /// The texture's place among those the instances were given.
    pub texture: usize,
    pub rect: Rect,
    pub at: [i32; 2],
    pub extent: Rect,
}

/// One instance of a stage: what it reads and the rectangle it writes, at its place in the output.
pub(super) struct Instance<'a> {
    pub stage: &'a dyn Stage,
    pub input: Cell,
    pub aux: Vec<Cell>,
    pub out: Rect,
    pub out_at: [i32; 2],
}

fn table_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

/// The work groups an instance spans each way.
fn groups(out: Rect) -> (u32, u32) {
    (out.w.div_ceil(8), out.h.div_ceil(8))
}

/// The most work groups a dispatch walks for an instance, against that instance's own: one dispatch
/// spans its largest instance for each, so a large one never makes every small one walk its groups.
const SPAN: u32 = 4;

/// `instances` in dispatches: each a set reading the same textures, of sizes within `SPAN` of the
/// span they share, with that span.
fn dispatches(instances: &[Instance]) -> Vec<(Vec<usize>, (u32, u32))> {
    let reads = |i: &Instance| (i.input.texture, i.aux.iter().map(|a| a.texture).collect::<Vec<_>>());
    let size = |i: &Instance| groups(i.out).0 * groups(i.out).1;
    let mut order: Vec<usize> = (0..instances.len()).collect();
    order.sort_by_cached_key(|&k| (reads(&instances[k]), std::cmp::Reverse(size(&instances[k]))));
    let mut sets: Vec<(Vec<usize>, (u32, u32))> = Vec::new();
    for k in order {
        let (i, (w, h)) = (&instances[k], groups(instances[k].out));
        if let Some((set, span)) = sets.last_mut() {
            let wider = (span.0.max(w), span.1.max(h));
            if reads(&instances[set[0]]) == reads(i) && wider.0 * wider.1 <= SPAN * size(i) {
                set.push(k);
                *span = wider;
                continue;
            }
        }
        sets.push((vec![k], (w, h)));
    }
    sets
}

impl Graph {
    fn instanced_layout(&mut self, arity: usize) -> &Layout {
        let device = &self.device;
        self.instanced.0.entry(arity).or_insert_with(|| {
            let mut entries = vec![
                table_entry(0),
                dispatch::texture_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: crate::texture::FORMAT, view_dimension: wgpu::TextureViewDimension::D2 },
                    count: None,
                },
                table_entry(3),
            ];
            if arity > 0 {
                entries.push(table_entry(4));
                entries.extend((0..arity).map(|k| dispatch::texture_entry(5 + k as u32)));
            }
            let group = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("stage of many"), entries: &entries });
            let pipeline = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("stage of many"), bind_group_layouts: &[Some(&group)], immediate_size: 0 });
            Layout { group, pipeline }
        })
    }

    fn instanced_pipeline(&mut self, stage: &dyn Stage, arity: usize) -> wgpu::ComputePipeline {
        let key = (stage.shader().as_ptr() as usize, stage.shader().len(), arity);
        if let Some(p) = self.instanced.1.get(&key) {
            return p.clone();
        }
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(stage.name()),
            source: wgpu::ShaderSource::Wgsl(instanced_source(stage.shader(), arity).into()),
        });
        let layout = self.instanced_layout(arity).pipeline.clone();
        let pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(stage.name()),
            layout: Some(&layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        self.instanced.1.insert(key, pipeline.clone());
        pipeline
    }

    /// Every instance of one kind of stage into `output`, in as few dispatches as their sizes and the
    /// textures they read allow; `textures` are those the cells name.
    pub(super) fn dispatch_instanced(&mut self, pass: &mut wgpu::ComputePass<'_>, instances: &[Instance], textures: &[FloatTexture], output: &FloatTexture) {
        let Some(first) = instances.first() else { return };
        let arity = first.aux.len();
        let pipeline = self.instanced_pipeline(first.stage, arity);
        let layout = self.instanced_layout(arity).group.clone();
        pass.set_pipeline(&pipeline);
        for (group, (wide, high)) in &dispatches(instances) {
            let (mut frames, mut params, mut places) = (Vec::new(), Vec::new(), Vec::new());
            for &k in group {
                let i = &instances[k];
                frames.push(InstanceFrame {
                    out_origin: [i.out.x, i.out.y],
                    in_origin: [i.input.rect.x, i.input.rect.y],
                    in_min: [i.input.extent.x, i.input.extent.y],
                    in_max: [i.input.extent.right(), i.input.extent.bottom()],
                    out_size: [i.out.w, i.out.h],
                    in_size: [i.input.rect.w, i.input.rect.h],
                    out_at: i.out_at,
                    in_at: i.input.at,
                });
                // A cell of the table holds a stage's parameters in 16-byte steps, as its uniform does.
                let bytes = i.stage.params();
                params.extend_from_slice(&bytes);
                params.resize(params.len() + bytes.len().div_ceil(16).max(1) * 16 - bytes.len(), 0);
                if arity > 0 {
                    let mut aux = [InstanceAux::default(); MAX_AUX];
                    for (f, a) in aux.iter_mut().zip(&i.aux) {
                        *f = InstanceAux { in_origin: [a.rect.x, a.rect.y], in_min: [a.extent.x, a.extent.y], in_max: [a.extent.right(), a.extent.bottom()], in_size: [a.rect.w as i32, a.rect.h as i32], in_at: a.at };
                    }
                    places.push(aux);
                }
            }
            let (frames, params, places) = if arity == 0 {
                let [frames, params] = self.uniforms([bytemuck::cast_slice(&frames), &params]);
                (frames, params, None)
            } else {
                let [frames, params, places] = self.uniforms([bytemuck::cast_slice(&frames), &params, bytemuck::cast_slice(&places)]);
                (frames, params, Some(places))
            };
            let buffer = &self.uniforms.buffer.clone();
            let bound = |(offset, size): (u64, u64)| wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer, offset, size: wgpu::BufferSize::new(size) });
            let lead = &instances[group[0]];
            let mut entries = vec![
                wgpu::BindGroupEntry { binding: 0, resource: bound(frames) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(textures[lead.input.texture].view()) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(output.view()) },
                wgpu::BindGroupEntry { binding: 3, resource: bound(params) },
            ];
            if let Some(places) = places {
                entries.push(wgpu::BindGroupEntry { binding: 4, resource: bound(places) });
                for (k, a) in lead.aux.iter().enumerate() {
                    entries.push(wgpu::BindGroupEntry { binding: 5 + k as u32, resource: wgpu::BindingResource::TextureView(textures[a.texture].view()) });
                }
            }
            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some(lead.stage.name()), layout: &layout, entries: &entries });
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(*wide, *high, group.len() as u32);
            self.dispatches += 1;
            self.pixels += group.iter().map(|&k| instances[k].out.area()).sum::<u64>();
        }
    }
}
