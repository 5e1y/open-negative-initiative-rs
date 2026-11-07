//! Auto De-dust's veil: the reading's map thresholded at the slider's notch on the GPU, with no CPU per notch.
//! `dust_mask` is the kernel's CPU reference.

use super::*;

/// The kernel's arguments, `dust.wgsl`'s.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DustParams {
    veil: [f32; 4],
    notch: u32,
    pad: [u32; 3],
}

/// One texel of the mask: the veil's colour straight, clear once `notch` reaches the texel.
fn dust_texel(t: u8, notch: u32, veil: [f32; 4]) -> [u8; 4] {
    let alpha = if u32::from(t) <= notch { 0.0 } else { veil[3] };
    let byte = |v: f32| (v * 255.0).round() as u8;
    [byte(veil[0]), byte(veil[1]), byte(veil[2]), byte(alpha)]
}

/// The kernel's CPU reference.
pub fn dust_mask(map: &[u8], notch: u32, veil: [f32; 4]) -> Vec<u8> {
    map.iter().flat_map(|&t| dust_texel(t, notch, veil)).collect()
}

/// The kernel on a device, and a reading's map on it.
pub struct DustKernel {
    pipeline: wgpu::ComputePipeline,
    map: wgpu::TextureView,
    size: (u32, u32),
}

impl DustKernel {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, (width, height): (u32, u32), texels: &[u8]) -> DustKernel {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("dust mask"), source: wgpu::ShaderSource::Wgsl(include_str!("dust.wgsl").into()) });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("dust mask"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let map = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("dust map"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width), rows_per_image: Some(height) };
        queue.write_texture(map.as_image_copy(), texels, layout, map.size());
        DustKernel { pipeline, map: map.create_view(&Default::default()), size: (width, height) }
    }

    /// The mask at `notch` into `target`, a texture of the map's size.
    pub fn run(&self, device: &wgpu::Device, queue: &wgpu::Queue, notch: u32, veil: [f32; 4], target: &wgpu::Texture) {
        use wgpu::util::DeviceExt;
        let k = DustParams { veil, notch, pad: [0; 3] };
        let block = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("dust params"), contents: bytemuck::bytes_of(&k), usage: wgpu::BufferUsages::UNIFORM });
        let view = target.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dust mask"),
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: block.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.map) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&view) },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(self.size.0.div_ceil(8), self.size.1.div_ceil(8), 1);
        }
        queue.submit([encoder.finish()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every notch a texel can hold around the ones the kernel is run at.
    fn map() -> Vec<u8> {
        [0u8, 1, 100, 101, 254, 255].repeat(6)
    }

    #[test]
    fn the_kernel_writes_what_its_cpu_reference_writes() {
        let Ok(oni_graph::Gpu { device, queue, .. }) = oni_graph::gpu() else {
            return eprintln!("no adapter: the dust kernel is not exercised");
        };
        let (texels, veil) = (map(), [0.0, 0.0, 0.0, 0.55]);
        let (w, h) = (6u32, 6u32);
        let kernel = DustKernel::new(&device, &queue, (w, h), &texels);
        let target = Gpu::target(&device, w, h);
        for notch in [0, 100, 254] {
            kernel.run(&device, &queue, notch, veil, &target);
            let got = read_back(&device, &queue, &target, w, h).expect("read back");
            let want = dust_mask(&texels, notch, veil);
            let worst = got.iter().zip(&want).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            assert!(worst <= 1, "at notch {notch}: {worst} levels apart");
            // Twin: the reference at another notch does not match the kernel.
            assert_ne!(got, dust_mask(&texels, notch + 1, veil), "at notch {notch}");
        }
    }

    #[test]
    fn a_texel_clears_from_its_own_notch_and_is_veiled_before() {
        let veil = [0.0, 0.0, 0.0, 0.55];
        let alpha = |t: u8, notch: u32| dust_texel(t, notch, veil)[3];
        assert_eq!(alpha(10, 9), 140, "twin: before its notch, veiled");
        assert_eq!(alpha(10, 10), 0, "clear from its notch");
        assert_eq!(alpha(255, 254), 140, "never clear at the slider's last notch");
        assert_eq!(dust_texel(255, 0, [1.0, 0.5, 0.0, 1.0])[..3], [255, 128, 0], "its colour is the design system's, straight");
    }
}
