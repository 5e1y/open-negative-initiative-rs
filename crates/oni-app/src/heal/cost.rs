//! The search's costs, `correctionCost`: one per candidate offset, on the GPU as Swift dispatches it,
//! with the CPU's `cost_map` as its reference and its fallback where no GPU answers.

use super::{Frame, Region};
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, Debug)]
pub(super) struct Candidate {
    pub offset: (f64, f64),
    pub cost: f32,
}

/// `correctionCost` over a grid of candidate offsets, row-major from the bottom as Core Image
/// dispatches it: a ring's cells sampled at each, the hole left out on either side.
#[allow(clippy::too_many_arguments)]
pub(super) fn cost_map(src: &Region, f: &Frame, half: (f64, f64), origin: (f64, f64), step: f64, grid_n: usize, size: usize, min_radius: f64) -> Vec<Candidate> {
    let e = f.extent;
    let hole = f.hole;
    let inside = |p: (f64, f64)| p.0 >= hole.x && p.0 <= hole.x + hole.w && p.1 >= hole.y && p.1 <= hole.y + hole.h;
    let cell = |j: usize| (j as f64 + 0.5) / grid_n as f64 * 2.0 - 1.0;
    // The stroke's side of every cell is the same for each candidate: sampled once, in the order summed.
    let cells: Vec<((f64, f64), Option<[f32; 3]>)> = (0..grid_n * grid_n)
        .map(|n| {
            let t = (cell(n % grid_n) * half.0, cell(n / grid_n) * half.1);
            let dest = (f.centre.0 + t.0, f.centre.1 + t.1);
            (t, (!inside(dest)).then(|| src.sample(dest.0, dest.1)))
        })
        .collect();
    let one = |index: usize| {
        let (i, j) = (index % size, index / size);
        let offset = (origin.0 + (i as f64 + 0.5) * step, origin.1 + (j as f64 + 0.5) * step);
        let cand = (f.centre.0 + offset.0, f.centre.1 + offset.1);
        if offset.0.hypot(offset.1) < min_radius
            || cand.0 - half.0 < e.x
            || cand.0 + half.0 > e.x + e.w
            || cand.1 - half.1 < e.y
            || cand.1 + half.1 > e.y + e.h
        {
            return Candidate { offset, cost: f32::INFINITY };
        }
        let (mut accum, mut counted) = ([0.0f32; 3], 0usize);
        for &(t, d) in &cells {
            let from = (cand.0 + t.0, cand.1 + t.1);
            let Some(d) = d.filter(|_| !inside(from)) else { continue };
            let s = src.sample(from.0, from.1);
            for k in 0..3 {
                let diff = d[k] - s[k];
                accum[k] += diff * diff / (d[k] * d[k] + 0.001);
            }
            counted += 1;
        }
        let cost = if counted == 0 { f32::INFINITY } else { (accum[0] + accum[1] + accum[2]) / counted as f32 };
        Candidate { offset, cost }
    };
    let count = size * size;
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let per = count.div_ceil(threads).max(1);
    let mut out = vec![Candidate { offset: (0.0, 0.0), cost: f32::INFINITY }; count];
    std::thread::scope(|scope| {
        for (k, chunk) in out.chunks_mut(per).enumerate() {
            scope.spawn(move || {
                for (n, slot) in chunk.iter_mut().enumerate() {
                    *slot = one(k * per + n);
                }
            });
        }
    });
    out
}

/// A search's source, laid on the GPU once for both its passes when the app's device is there.
pub(super) struct Costs<'a> {
    src: &'a Region,
    laid: Option<Laid>,
}

struct Laid {
    gpu: &'static Gpu,
    pixels: wgpu::Buffer,
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    door: Arc<crate::render::Door>,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    region: [i32; 4],
    frame: [f32; 2],
    centre: [f32; 2],
    half: [f32; 2],
    origin: [f32; 2],
    hole: [f32; 4],
    extent: [f32; 4],
    step: f32,
    min_radius: f32,
    grid_n: u32,
    size: u32,
}

const SHADER: &str = include_str!("cost.wgsl");

fn gpu() -> Option<&'static Gpu> {
    static GPU: OnceLock<Option<Gpu>> = OnceLock::new();
    GPU.get_or_init(|| {
        let (device, queue, door) = crate::render::app_gpu()?;
        Some(Gpu::on(device, queue, door))
    })
    .as_ref()
}

impl Gpu {
    fn on(device: wgpu::Device, queue: wgpu::Queue, door: Arc<crate::render::Door>) -> Gpu {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("correction cost"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let uniform = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("correction cost"), entries: &[uniform, storage(1, true), storage(2, false)] });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("correction cost"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("correction cost"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Gpu { device, queue, door, pipeline, layout }
    }
}

impl<'a> Costs<'a> {
    /// The source's colours laid once; on the CPU alone where no GPU answers or it is too large to bind.
    pub(super) fn of(src: &'a Region) -> Costs<'a> {
        Costs::on(gpu(), src)
    }

    fn on(gpu: Option<&'static Gpu>, src: &'a Region) -> Costs<'a> {
        let laid = gpu.and_then(|gpu| {
            let bytes = (src.pixels.len() * 16) as u64;
            if bytes == 0 || bytes > gpu.device.limits().max_storage_buffer_binding_size {
                return None;
            }
            use wgpu::util::DeviceExt;
            let pixels = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("search source"), contents: bytemuck::cast_slice(&src.pixels), usage: wgpu::BufferUsages::STORAGE });
            Some(Laid { gpu, pixels })
        });
        Costs { src, laid }
    }

    /// 's candidates, from the GPU when the source is laid there.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn map(&self, f: &Frame, half: (f64, f64), origin: (f64, f64), step: f64, grid_n: usize, size: usize, min_radius: f64) -> Vec<Candidate> {
        let costs = self.laid.as_ref().and_then(|laid| laid.costs(self.src, f, half, origin, step, grid_n, size, min_radius));
        let Some(costs) = costs else { return cost_map(self.src, f, half, origin, step, grid_n, size, min_radius) };
        costs
            .into_iter()
            .enumerate()
            .map(|(index, cost)| {
                let (i, j) = (index % size, index / size);
                Candidate { offset: (origin.0 + (i as f64 + 0.5) * step, origin.1 + (j as f64 + 0.5) * step), cost }
            })
            .collect()
    }
}

impl Laid {
    #[allow(clippy::too_many_arguments)]
    fn costs(&self, src: &Region, f: &Frame, half: (f64, f64), origin: (f64, f64), step: f64, grid_n: usize, size: usize, min_radius: f64) -> Option<Vec<f32>> {
        let gpu = self.gpu;
        let count = size * size;
        let params = Params {
            region: [src.x as i32, src.y as i32, src.width as i32, src.height as i32],
            frame: [src.frame_width as f32, src.frame_height as f32],
            centre: [f.centre.0 as f32, f.centre.1 as f32],
            half: [half.0 as f32, half.1 as f32],
            origin: [origin.0 as f32, origin.1 as f32],
            hole: [f.hole.x as f32, f.hole.y as f32, f.hole.w as f32, f.hole.h as f32],
            extent: [f.extent.x as f32, f.extent.y as f32, f.extent.w as f32, f.extent.h as f32],
            step: step as f32,
            min_radius: min_radius as f32,
            grid_n: grid_n as u32,
            size: size as u32,
        };
        use wgpu::util::DeviceExt;
        let uniform = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("cost params"), contents: bytemuck::bytes_of(&params), usage: wgpu::BufferUsages::UNIFORM });
        let bytes = (count * 4) as u64;
        let out = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("costs"), size: bytes, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
        let read = gpu.device.create_buffer(&wgpu::BufferDescriptor { label: Some("costs read"), size: bytes, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("correction cost"),
            layout: &gpu.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.pixels.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: out.as_entire_binding() },
            ],
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&gpu.pipeline);
            pass.set_bind_group(0, &group, &[]);
            let groups = (count as u32).div_ceil(64);
            pass.dispatch_workgroups(groups.min(65_535), groups.div_ceil(65_535), 1);
        }
        encoder.copy_buffer_to_buffer(&out, 0, &read, 0, bytes);
        gpu.door.enter().ok()?;
        gpu.queue.submit([encoder.finish()]);
        gpu.door.leave();
        read.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let mapped = read.slice(..).get_mapped_range().ok()?;
        let costs = bytemuck::cast_slice::<u8, f32>(&mapped).to_vec();
        Some(costs)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Rect;
    use super::*;

    fn region() -> Region {
        let (width, height) = (300, 260);
        let pixels = (0..width * height)
            .map(|i| {
                let (x, y) = ((i % width) as f32, (i / width) as f32);
                [0.3 + 0.2 * (x * 0.07).sin() * (y * 0.05).cos(), 0.4 + 0.1 * (x * 0.03 + y * 0.02).sin(), 0.5 + 0.15 * (y * 0.09).sin(), 1.0]
            })
            .collect();
        Region { x: 40, y: 30, width, height, pixels, frame_width: 400.0, frame_height: 340.0 }
    }

    fn frame() -> Frame {
        let hole = Rect { x: 180.0, y: 160.0, w: 24.0, h: 20.0 };
        Frame { extent: Rect { x: 0.0, y: 0.0, w: 400.0, h: 340.0 }, tile: hole, centre: (192.0, 170.0), hole }
    }

    fn gpu_or_skip() -> Option<&'static Gpu> {
        let device = crate::render::Device::open().ok()?;
        Some(Box::leak(Box::new(Gpu::on(device.device, device.queue, Arc::default()))))
    }

    #[test]
    fn the_gpu_costs_are_the_cpu_reference_s() {
        let Some(gpu) = gpu_or_skip() else { return eprintln!("no adapter: the GPU costs are not exercised") };
        let (src, f) = (region(), frame());
        let costs = Costs::on(Some(gpu), &src);
        assert!(costs.laid.is_some());
        let (half, origin, size) = ((30.0, 26.0), (-60.0, -60.0), 41);
        let gpu_map = costs.map(&f, half, origin, 3.0, 16, size, 40.0);
        let cpu_map = cost_map(&src, &f, half, origin, 3.0, 16, size, 40.0);
        let mut finite = 0;
        for (g, c) in gpu_map.iter().zip(&cpu_map) {
            assert_eq!(g.offset, c.offset);
            assert_eq!(g.cost.is_finite(), c.cost.is_finite(), "at {:?}", c.offset);
            if c.cost.is_finite() {
                finite += 1;
                assert!((g.cost - c.cost).abs() <= 1e-3 * c.cost.abs().max(1e-3), "at {:?}: {} against {}", c.offset, g.cost, c.cost);
            }
        }
        assert!(finite > 500, "the grid reached few candidates: {finite}");
        // Twin: the same grid with another ring size scores otherwise, so the comparison can fail.
        let other = cost_map(&src, &f, (20.0, 26.0), origin, 3.0, 16, size, 40.0);
        assert!(gpu_map.iter().zip(&other).any(|(g, o)| o.cost.is_finite() && (g.cost - o.cost).abs() > 1e-2));
    }
}

