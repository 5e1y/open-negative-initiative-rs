//! The interface renderer's own changes (vendor/femtovg, vendor/i-slint-renderer-femtovg): a frame of
//! many passes submitted in slices, and a colourised glyph drawn as a mask instead of a copy.

use femtovg::renderer::WGPURenderer;
use femtovg::{Canvas, Color, CompositeOperation, ImageFlags, ImageId, Paint, Path, PixelFormat, RenderTarget};

const CELL: u32 = 8;
const COLUMNS: u32 = 8;

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    oni_graph::gpu().ok().map(|g| (g.device, g.queue))
}

fn colour(i: u32) -> [u8; 3] {
    [(i * 37 % 256) as u8, (i * 91 % 256) as u8, (255 - i * 53 % 256) as u8]
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
    let mut path = Path::new();
    path.rect(x, y, w, h);
    path
}

/// What `draw` paints on a black `w`×`h` window, read back as RGBA rows.
fn drawn(device: &wgpu::Device, queue: &wgpu::Queue, (w, h): (u32, u32), draw: impl FnOnce(&mut Canvas<WGPURenderer>)) -> Vec<u8> {
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
    draw(&mut canvas);
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

/// `layers` cells, each filled in an image of its own (a pass into the image, a pass back).
fn layered(device: &wgpu::Device, queue: &wgpu::Queue, layers: u32) -> Vec<u8> {
    drawn(device, queue, (COLUMNS * CELL, layers.div_ceil(COLUMNS) * CELL), |canvas| {
        for i in 0..layers {
            let image = canvas.create_image_empty(CELL as usize, CELL as usize, PixelFormat::Rgba8, ImageFlags::empty()).unwrap();
            canvas.set_render_target(RenderTarget::Image(image));
            canvas.clear_rect(0, 0, CELL, CELL, Color::rgba(0, 0, 0, 0));
            let [r, g, b] = colour(i);
            canvas.fill_path(&rect(0.0, 0.0, CELL as f32, CELL as f32), &Paint::color(Color::rgb(r, g, b)));
            canvas.set_render_target(RenderTarget::Screen);
            let (x, y) = ((i % COLUMNS * CELL) as f32, (i / COLUMNS * CELL) as f32);
            canvas.fill_path(&rect(x, y, CELL as f32, CELL as f32), &Paint::image(image, x, y, CELL as f32, CELL as f32, 0.0, 1.0));
        }
    })
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
    let pixels = layered(&device, &queue, 40);
    assert_eq!(misplaced(&pixels, 40, colour), Vec::<u32>::new());
    let alone = layered(&device, &queue, 8);
    assert_eq!(pixels[..alone.len()], alone[..], "the first row drawn in slices differs from it drawn in one buffer");
}

#[test]
fn a_cell_in_another_cells_colour_is_seen() {
    let Some((device, queue)) = device() else { return };
    let pixels = layered(&device, &queue, 40);
    assert_eq!(misplaced(&pixels, 40, |i| colour((i + 1) % 40)).len(), 40);
}

const INK: Color = Color { r: 0.9, g: 0.5, b: 0.2, a: 0.8 };

/// A black glyph, opaque on its left half and half transparent on a column, as Slint uploads icons.
fn glyph(canvas: &mut Canvas<WGPURenderer>) -> ImageId {
    let image = canvas.create_image_empty(CELL as usize, CELL as usize, PixelFormat::Rgba8, ImageFlags::PREMULTIPLIED).unwrap();
    canvas.set_render_target(RenderTarget::Image(image));
    canvas.clear_rect(0, 0, CELL, CELL, Color::rgba(0, 0, 0, 0));
    canvas.fill_path(&rect(0.0, 0.0, 4.0, CELL as f32), &Paint::color(Color::rgba(0, 0, 0, 255)));
    canvas.fill_path(&rect(5.0, 0.0, 1.0, CELL as f32), &Paint::color(Color::rgba(0, 0, 0, 128)));
    canvas.set_render_target(RenderTarget::Screen);
    image
}

/// The glyph colourised as Slint's renderer did before the mask: copied, then the ink filled where it shows.
fn copied(canvas: &mut Canvas<WGPURenderer>) {
    let source = glyph(canvas);
    let image = canvas.create_image_empty(CELL as usize, CELL as usize, PixelFormat::Rgba8, ImageFlags::PREMULTIPLIED).unwrap();
    let whole = rect(0.0, 0.0, CELL as f32, CELL as f32);
    canvas.save();
    canvas.set_render_target(RenderTarget::Image(image));
    canvas.global_composite_operation(CompositeOperation::Copy);
    canvas.fill_path(&whole, &Paint::image(source, 0.0, 0.0, CELL as f32, CELL as f32, 0.0, 1.0));
    canvas.global_composite_operation(CompositeOperation::SourceIn);
    canvas.fill_path(&whole, &Paint::color(INK).with_anti_alias(false));
    canvas.set_render_target(RenderTarget::Screen);
    canvas.restore();
    canvas.fill_path(&whole, &Paint::image(image, 0.0, 0.0, CELL as f32, CELL as f32, 0.0, 1.0));
}

fn painted(canvas: &mut Canvas<WGPURenderer>, paint: fn(ImageId) -> Paint) {
    let source = glyph(canvas);
    canvas.fill_path(&rect(0.0, 0.0, CELL as f32, CELL as f32), &paint(source));
}

fn farthest(a: &[u8], b: &[u8]) -> u8 {
    a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap()
}

#[test]
fn a_masked_glyph_draws_as_its_colourised_copy() {
    let Some((device, queue)) = device() else { return };
    let size = (CELL, CELL);
    let copy = drawn(&device, &queue, size, copied);
    let mask = drawn(&device, &queue, size, |canvas| painted(canvas, |id| Paint::image_mask(id, 0.0, 0.0, 8.0, 8.0, 0.0, INK)));
    assert!(farthest(&copy, &mask) <= 1, "{copy:?}\n{mask:?}");
}

#[test]
fn a_tinted_glyph_is_told_from_its_colourised_copy() {
    let Some((device, queue)) = device() else { return };
    let size = (CELL, CELL);
    let copy = drawn(&device, &queue, size, copied);
    let tint = drawn(&device, &queue, size, |canvas| painted(canvas, |id| Paint::image_tint(id, 0.0, 0.0, 8.0, 8.0, 0.0, INK)));
    assert!(farthest(&copy, &tint) > 100);
}
