//! Auto De-dust's mask: the reading's map thresholded at the slider's notch on the GPU, with no CPU per notch.
//! `dust_mask` is the kernel's CPU reference.

use super::*;

/// The kernel's arguments, `dust.wgsl`'s.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DustParams {
    lit: [f32; 4],
    past: f32,
    notch: u32,
    pad: [u32; 2],
}

/// The share of the lit colour what is past the window shows at, dust lit whole: the rest is left alone.
pub const PAST_SHARE: f32 = 110.0 / 255.0;

/// One texel of the mask: its colour straight, its alpha by what shows at `notch`.
fn dust_texel(t: [u8; 2], notch: u32, lit: [f32; 3]) -> [u8; 4] {
    let alpha = if u32::from(t[1]) <= notch {
        1.0
    } else if u32::from(t[0]) <= notch {
        PAST_SHARE
    } else {
        0.0
    };
    let byte = |v: f32| (v * 255.0).round() as u8;
    [byte(lit[0]), byte(lit[1]), byte(lit[2]), byte(alpha)]
}

/// The kernel's CPU reference.
pub fn dust_mask(map: &[[u8; 2]], notch: u32, lit: [f32; 3]) -> Vec<u8> {
    map.iter().flat_map(|&t| dust_texel(t, notch, lit)).collect()
}

/// The kernel on a device, and a reading's map on it.
pub struct DustKernel {
    pipeline: wgpu::ComputePipeline,
    map: wgpu::TextureView,
    size: (u32, u32),
}

impl DustKernel {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, (width, height): (u32, u32), texels: &[[u8; 2]]) -> DustKernel {
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
            format: wgpu::TextureFormat::Rg8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 2), rows_per_image: Some(height) };
        queue.write_texture(map.as_image_copy(), bytemuck::cast_slice(texels), layout, map.size());
        DustKernel { pipeline, map: map.create_view(&Default::default()), size: (width, height) }
    }

    /// The mask at `notch` into `target`, a texture of the map's size.
    pub fn run(&self, device: &wgpu::Device, queue: &wgpu::Queue, notch: u32, lit: [f32; 3], target: &wgpu::Texture) {
        use wgpu::util::DeviceExt;
        let k = DustParams { lit: [lit[0], lit[1], lit[2], 1.0], past: PAST_SHARE, notch, pad: [0; 2] };
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

    /// Every order a texel's two notches can stand in around the ones the kernel is run at.
    fn map() -> Vec<[u8; 2]> {
        let notches = [0u8, 1, 100, 101, 254, 255];
        notches.iter().flat_map(|&a| notches.iter().map(move |&b| [a, b])).collect()
    }

    #[test]
    fn the_kernel_writes_what_its_cpu_reference_writes() {
        let Ok(oni_graph::Gpu { device, queue, .. }) = oni_graph::gpu() else {
            return eprintln!("no adapter: the dust kernel is not exercised");
        };
        let (texels, lit) = (map(), [1.0, 0.584, 0.0]);
        let (w, h) = (6u32, 6u32);
        let kernel = DustKernel::new(&device, &queue, (w, h), &texels);
        let target = Gpu::target(&device, w, h);
        for notch in [0, 100, 254] {
            kernel.run(&device, &queue, notch, lit, &target);
            let got = read_back(&device, &queue, &target, w, h).expect("read back");
            let want = dust_mask(&texels, notch, lit);
            let worst = got.iter().zip(&want).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            assert!(worst <= 1, "at notch {notch}: {worst} levels apart");
            // Twin: the reference at another notch does not match the kernel.
            assert_ne!(got, dust_mask(&texels, notch + 1, lit), "at notch {notch}");
        }
    }

    #[test]
    fn a_texel_shows_from_its_own_notch_dust_over_what_is_past() {
        let lit = [1.0, 0.5, 0.0];
        let alpha = |t: [u8; 2], notch: u32| dust_texel(t, notch, lit)[3];
        let past = (PAST_SHARE * 255.0).round() as u8;
        assert_eq!(alpha([10, 20], 9), 0, "twin: below both, clear");
        assert_eq!(alpha([10, 20], 10), past);
        assert_eq!(alpha([10, 20], 20), 255);
        assert_eq!(alpha([255, 255], 254), 0, "never shows at the slider's last notch");
        assert_eq!(dust_texel([0, 0], 0, lit)[..3], [255, 128, 0], "its colour is the design system's, straight");
    }
}
