use super::*;

#[test]
fn the_matrices_keep_white_and_match_the_published_ones() {
    for screen in [Screen::Srgb, Screen::DisplayP3] {
        let m = screen.matrix();
        for r in m {
            assert!((r.iter().sum::<f32>() - 1.0).abs() < 1e-5, "{screen:?} moves white");
        }
    }
    // BT.2087's Rec. 2020 to Rec. 709 first row.
    let m = Screen::Srgb.matrix();
    assert!((m[0][0] - 1.6605).abs() < 1e-3 && (m[0][1] + 0.5876).abs() < 1e-3);
    // Twin: P3 is a wider target, so its red row pulls less than sRGB's.
    assert!(Screen::DisplayP3.matrix()[0][0] < m[0][0] - 0.2);
}

#[test]
fn the_dither_breaks_a_ramp_into_noise_around_the_right_level() {
    let (w, h) = (256u32, 64u32);
    let grey: Vec<[f32; 4]> = (0..w * h).map(|_| [0.18, 0.18, 0.18, 1.0]).collect();
    let k = Params::new(Screen::Srgb, w, h, 7);
    let out = encode(&grey, &k);
    let levels: Vec<u8> = out.chunks(4).map(|p| p[0]).collect();
    let mean = levels.iter().map(|&v| v as f64).sum::<f64>() / levels.len() as f64;
    let exact = transfer(0.18) as f64 * 255.0;
    assert!((mean - exact).abs() < 0.05, "mean {mean} against {exact}");
    assert!(levels.iter().all(|&v| (v as f64 - exact).abs() < 2.0), "one step of noise at most either side");
    // Twin: without dither every pixel lands on one level, which is the banding the noise breaks.
    let flat = encode(&grey, &Params { dither: 0.0, ..k });
    let first = flat[0];
    assert!(flat.chunks(4).all(|p| p[0] == first));
    assert!(levels.iter().any(|&v| v != first));
}

/// The kernel on the first adapter wgpu finds, lavapipe included; skipped where there is none.
#[test]
fn the_kernel_writes_what_its_cpu_reference_writes() {
    let instance = wgpu::Instance::default();
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no adapter: the display kernel is not exercised");
        return;
    };
    let Ok((device, queue)) = pollster::block_on(adapter.request_device(&Default::default())) else { return };
    let (w, h) = (64u32, 16u32);
    let pixels: Vec<[f32; 4]> = (0..w * h)
        .map(|i| {
            let t = (i % w) as f32 / (w - 1) as f32;
            [t * 1.2 - 0.1, t * t, 1.0 - t, 1.0]
        })
        .collect();
    let gpu = Gpu::new(&device);
    let source = upload(&device, &queue, &pixels, w, h).create_view(&Default::default());
    for screen in [Screen::Srgb, Screen::DisplayP3] {
        let k = Params::new(screen, w, h, 3);
        let target = Gpu::target(&device, w, h);
        gpu.run(&device, &queue, &source, &k, &target);
        let got = read_back(&device, &queue, &target, w, h);
        let want = encode(&pixels, &k);
        let worst = got.iter().zip(&want).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
        assert!(worst <= 1, "{screen:?}: {worst} levels apart");
        // Twin: the reference with the other screen's matrix does not match the kernel.
        let other = if screen == Screen::Srgb { Screen::DisplayP3 } else { Screen::Srgb };
        let wrong = encode(&pixels, &Params::new(other, w, h, 3));
        assert!(got.iter().zip(&wrong).any(|(a, b)| a.abs_diff(*b) > 2));
    }
}

/// The zebra on the GPU against its CPU reference, on colours sRGB cannot hold and one it can.
#[test]
fn the_zebra_stripes_what_the_screen_cannot_show_as_its_reference_does() {
    let instance = wgpu::Instance::default();
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no adapter: the zebra is not exercised");
        return;
    };
    let Ok((device, queue)) = pollster::block_on(adapter.request_device(&Default::default())) else { return };
    let (w, h) = (48u32, 20u32);
    // The left half a saturated Rec. 2020 green sRGB cannot show, the right half a grey it can.
    let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| if i % w < w / 2 { [0.05, 0.6, 0.05, 1.0] } else { [0.3, 0.3, 0.3, 1.0] }).collect();
    let gpu = Gpu::new(&device);
    let source = upload(&device, &queue, &pixels, w, h).create_view(&Default::default());
    let zebra = Zebra { origin: (10.0, 4.0), stretch: 2.0, canvas_height: 60.0, phase: 3.0 };
    let k = Params::new(Screen::Srgb, w, h, 3).striped(zebra);
    let target = Gpu::target(&device, w, h);
    gpu.run(&device, &queue, &source, &k, &target);
    let got = read_back(&device, &queue, &target, w, h);
    let want = encode(&pixels, &k);
    assert!(got.iter().zip(&want).all(|(a, b)| a.abs_diff(*b) <= 1));
    let plain = encode(&pixels, &Params::new(Screen::Srgb, w, h, 3));
    let changed = |half: std::ops::Range<u32>| (0..h).flat_map(|y| half.clone().map(move |x| (y * w + x) as usize * 4)).filter(|&i| want[i..i + 3] != plain[i..i + 3]).count();
    assert!(changed(0..w / 2) > 0, "the green the screen cannot show is striped");
    assert_eq!(changed(w / 2..w), 0, "the grey it can show is left alone");
    // Twin: another phase moves the stripes, so a kernel ignoring the clock would be seen.
    let moved = encode(&pixels, &Params::new(Screen::Srgb, w, h, 3).striped(Zebra { phase: 7.5, ..zebra }));
    assert_ne!(moved, want);
}

/// The GPU count against `Histogram::of` on the same shown bytes, sampled on every second row and column.
#[test]
fn the_histogram_kernel_counts_what_the_cpu_counts() {
    let instance = wgpu::Instance::default();
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no adapter: the histogram kernel is not exercised");
        return;
    };
    let Ok((device, queue)) = pollster::block_on(adapter.request_device(&Default::default())) else { return };
    let (w, h) = (37u32, 23u32);
    let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| [(i % w) as f32 / w as f32, (i / w) as f32 / h as f32, 0.4, 1.0]).collect();
    let gpu = Gpu::new(&device);
    let source = upload(&device, &queue, &pixels, w, h).create_view(&Default::default());
    let target = Gpu::target(&device, w, h);
    gpu.run(&device, &queue, &source, &Params::new(Screen::Srgb, w, h, 9), &target);
    assert!(gpu.count(&device, &queue, &target));
    assert!(!gpu.count(&device, &queue, &target), "a second count waits for the first to be read");
    device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let counted = gpu.histogram().expect("the count landed");
    let shown = read_back(&device, &queue, &target, w, h);
    let sampled: Vec<u8> = (0..h as usize)
        .step_by(2)
        .flat_map(|y| (0..w as usize).step_by(2).map(move |x| (y * w as usize + x) * 4))
        .flat_map(|i| shown[i..i + 4].to_vec())
        .collect();
    assert_eq!(counted, crate::render::Histogram::of(&sampled));
    // Twin: every pixel counted is another histogram, so a kernel that skips no row would be seen.
    assert_ne!(counted, crate::render::Histogram::of(&shown));
}

fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, w: u32, h: u32) -> Vec<u8> {
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
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let mapped = slice.get_mapped_range().expect("mapped range");
    (0..h as usize).flat_map(|y| mapped[y * row as usize..y * row as usize + w as usize * 4].to_vec()).collect()
}
