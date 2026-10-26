//! The display kernels on a device: the canvas encoded once a frame, its histogram on its own pace.

use super::*;

pub const KERNEL: &str = include_str!("encode.wgsl");

/// Counts the picture as `histogram::measure` does, its CPU reference.
pub const HISTOGRAM_KERNEL: &str = include_str!("histogram.wgsl");

pub(super) const BINS: u64 = crate::histogram::BINS as u64 * 4 * 4;

/// The kernel's arguments: the screen's matrices there and back, the copy's grid and the picture's size.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Measure {
    to: [[f32; 4]; 3],
    back: [[f32; 4]; 3],
    grid: [u32; 4],
    clear: [u32; 4],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mapping {
    Idle,
    Waiting,
    Mapped,
}

/// The kernels on a device: one dispatch per frame into a texture Slint can import, and on its own
/// pace the histogram of what it wrote, read back without waiting.
pub struct Gpu {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    histogram: wgpu::ComputePipeline,
    counts: wgpu::Buffer,
    staging: wgpu::Buffer,
    mapping: std::sync::Arc<std::sync::Mutex<Mapping>>,
}

impl Gpu {
    pub fn new(device: &wgpu::Device) -> Gpu {
        let entry = |binding, ty| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty, count: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("display"),
            entries: &[
                entry(0, wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }),
                // rgba32float is not filterable without a feature: the kernel only loads texels.
                entry(1, wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                }),
                entry(2, wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    view_dimension: wgpu::TextureViewDimension::D2,
                }),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("display"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let compute = |label, source: &str, layout: Option<&wgpu::PipelineLayout>| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout,
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let pipeline = compute("display", KERNEL, Some(&pipeline_layout));
        let histogram = compute("histogram", HISTOGRAM_KERNEL, None);
        let buffer = |label, usage| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: BINS, usage, mapped_at_creation: false });
        let counts = buffer("histogram counts", wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST);
        let staging = buffer("histogram readback", wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
        Gpu { pipeline, layout, histogram, counts, staging, mapping: std::sync::Arc::new(std::sync::Mutex::new(Mapping::Idle)) }
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

    /// Encodes `source`, an rgba32float texture the size of `target`, into `target`.
    pub fn run(&self, device: &wgpu::Device, queue: &wgpu::Queue, source: &wgpu::TextureView, k: &Params, target: &wgpu::Texture) {
        use wgpu::util::DeviceExt;
        let block = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("display params"),
            contents: bytemuck::bytes_of(k),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let view = target.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("display"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: block.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(source) },
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

    /// Counts `picture`, `width` by `height`, its clear samples left out on a `sheet`, unless the last
    /// count is still being read; says whether it did. `read` runs once the count can be read, on whichever thread the device was polled.
    #[allow(clippy::too_many_arguments)]
    pub fn count(&self, device: &wgpu::Device, queue: &wgpu::Queue, picture: &wgpu::TextureView, (width, height): (u32, u32), screen: Screen, sheet: bool, read: impl FnOnce() + Send + 'static) -> bool {
        use wgpu::util::DeviceExt;
        {
            let mut m = self.mapping.lock().expect("histogram mapping");
            if *m != Mapping::Idle {
                return false;
            }
            *m = Mapping::Waiting;
        }
        let (gw, gh) = crate::histogram::grid(width as usize, height as usize);
        let row = |r: [f32; 3]| [r[0], r[1], r[2], 0.0];
        let k = Measure { to: screen.matrix().map(row), back: screen.back().map(row), grid: [gw as u32, gh as u32, width, height], clear: [u32::from(sheet), 0, 0, 0] };
        let block = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("histogram params"), contents: bytemuck::bytes_of(&k), usage: wgpu::BufferUsages::UNIFORM });
        let counting = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("histogram"),
            layout: &self.histogram.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: block.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(picture) },
                wgpu::BindGroupEntry { binding: 2, resource: self.counts.as_entire_binding() },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.clear_buffer(&self.counts, 0, None);
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.histogram);
            pass.set_bind_group(0, &counting, &[]);
            pass.dispatch_workgroups((gw as u32).div_ceil(8), (gh as u32).div_ceil(8), 1);
        }
        encoder.copy_buffer_to_buffer(&self.counts, 0, &self.staging, 0, BINS);
        queue.submit([encoder.finish()]);
        let mapping = self.mapping.clone();
        self.staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            *mapping.lock().expect("histogram mapping") = if r.is_ok() { Mapping::Mapped } else { Mapping::Idle };
            read();
        });
        true
    }

    /// A count is on the GPU or waits to be read.
    pub fn counting(&self) -> bool {
        *self.mapping.lock().expect("histogram mapping") != Mapping::Idle
    }

    /// The last count once its readback has landed, which a `device.poll` delivers; none before.
    pub fn histogram(&self) -> Option<oni_measure::histogram::Histogram> {
        let mut m = self.mapping.lock().expect("histogram mapping");
        if *m != Mapping::Mapped {
            return None;
        }
        let bins = {
            let range = self.staging.slice(..).get_mapped_range().ok()?;
            bytemuck::cast_slice::<u8, [u32; 4]>(&range).to_vec()
        };
        self.staging.unmap();
        *m = Mapping::Idle;
        let rgb = bins.iter().map(|b| [b[0] as f32, b[1] as f32, b[2] as f32]).collect();
        let luma = bins.iter().map(|b| b[3] as f32).collect();
        Some(oni_measure::histogram::Histogram { rgb, luma, ends: None })
    }
}

/// The `w` × `h` corner of an rgba8 texture read back, rows packed: the canvas on a window that cannot
/// take the GPU's textures, and the checks' reading of the kernel; none when the GPU fails to deliver it.
pub fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, w: u32, h: u32) -> Option<Vec<u8>> {
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
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
    rx.recv().ok()?.ok()?;
    let mapped = slice.get_mapped_range().ok()?;
    Some((0..h as usize).flat_map(|y| mapped[y * row as usize..y * row as usize + w as usize * 4].to_vec()).collect())
}
