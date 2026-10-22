//! The interface renderer submits a frame of many passes in slices (vendor/femtovg): every layer of a
//! frame past the first slice must still land in its place with its own colour.

use femtovg::renderer::WGPURenderer;
use femtovg::{Canvas, Color, ImageFlags, Paint, Path, PixelFormat, RenderTarget};

const CELL: u32 = 8;
const COLUMNS: u32 = 8;

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
    pollster::block_on(adapter.request_device(&Default::default())).ok()
}

fn colour(i: u32) -> [u8; 3] {
    [(i * 37 % 256) as u8, (i * 91 % 256) as u8, (255 - i * 53 % 256) as u8]
}

/// Draws `layers` cells, each filled in an image of its own (a pass into the image, a pass back), and
/// reads the window back as RGBA rows.
fn drawn(device: &wgpu::Device, queue: &wgpu::Queue, layers: u32) -> Vec<u8> {
    let (w, h) = (COLUMNS * CELL, layers.div_ceil(COLUMNS) * CELL);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut canvas = Canvas::new(WGPURenderer::new(device.clone(), queue.clone())).unwrap();
    canvas.set_size(w, h, 1.0);
    canvas.clear_rect(0, 0, w, h, Color::rgba(0, 0, 0, 255));
    for i in 0..layers {
        let image = canvas.create_image_empty(CELL as usize, CELL as usize, PixelFormat::Rgba8, ImageFlags::empty()).unwrap();
        canvas.set_render_target(RenderTarget::Image(image));
        canvas.clear_rect(0, 0, CELL, CELL, Color::rgba(0, 0, 0, 0));
        let mut cell = Path::new();
        cell.rect(0.0, 0.0, CELL as f32, CELL as f32);
        let [r, g, b] = colour(i);
        canvas.fill_path(&cell, &Paint::color(Color::rgb(r, g, b)));
        canvas.set_render_target(RenderTarget::Screen);
        let (x, y) = ((i % COLUMNS * CELL) as f32, (i / COLUMNS * CELL) as f32);
        let mut placed = Path::new();
        placed.rect(x, y, CELL as f32, CELL as f32);
        canvas.fill_path(&placed, &Paint::image(image, x, y, CELL as f32, CELL as f32, 0.0, 1.0));
    }
    queue.submit(canvas.flush_to_output(&target));
    let row = (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) },
        },
        target.size(),
    );
    queue.submit([encoder.finish()]);
    readback.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let mapped = readback.slice(..).get_mapped_range().unwrap();
    mapped.chunks(row as usize).flat_map(|line| line[..(w * 4) as usize].to_vec()).collect()
}

/// The cells whose middle pixel is not the colour `expected` gives them.
fn misplaced(pixels: &[u8], layers: u32, expected: impl Fn(u32) -> [u8; 3]) -> Vec<u32> {
    let width = (COLUMNS * CELL) as usize;
    (0..layers)
        .filter(|&i| {
            let (x, y) = ((i % COLUMNS * CELL + CELL / 2) as usize, (i / COLUMNS * CELL + CELL / 2) as usize);
            let at = (y * width + x) * 4;
            pixels[at..at + 3] != expected(i)
        })
        .collect()
}

#[test]
fn a_frame_of_many_passes_lands_whole() {
    let Some((device, queue)) = device() else { return };
    let pixels = drawn(&device, &queue, 40);
    assert_eq!(misplaced(&pixels, 40, colour), Vec::<u32>::new());
    let alone = drawn(&device, &queue, 8);
    assert_eq!(pixels[..alone.len()], alone[..], "the first row drawn in slices differs from it drawn in one buffer");
}

#[test]
fn a_cell_in_another_cells_colour_is_seen() {
    let Some((device, queue)) = device() else { return };
    let pixels = drawn(&device, &queue, 40);
    assert_eq!(misplaced(&pixels, 40, |i| colour((i + 1) % 40)).len(), 40);
}
