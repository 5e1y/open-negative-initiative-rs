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

/// The kernel on the process's GPU, lavapipe included; skipped where there is none.
#[test]
fn the_kernel_writes_what_its_cpu_reference_writes() {
    let Ok(oni_graph::Gpu { device, queue, .. }) = oni_graph::gpu() else {
        eprintln!("no adapter: the display kernel is not exercised");
        return;
    };
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
        let got = read_back(&device, &queue, &target, w, h).expect("read back");
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
    let Ok(oni_graph::Gpu { device, queue, .. }) = oni_graph::gpu() else {
        eprintln!("no adapter: the zebra is not exercised");
        return;
    };
    let (w, h) = (48u32, 20u32);
    // The left half a saturated Rec. 2020 green sRGB cannot show, the right half a grey it can.
    let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| if i % w < w / 2 { [0.05, 0.6, 0.05, 1.0] } else { [0.3, 0.3, 0.3, 1.0] }).collect();
    let gpu = Gpu::new(&device);
    let source = upload(&device, &queue, &pixels, w, h).create_view(&Default::default());
    let zebra = Zebra { origin: (10.0, 4.0), stretch: 2.0, canvas_height: 60.0, phase: 3.0 };
    let k = Params::new(Screen::Srgb, w, h, 3).striped(zebra);
    let target = Gpu::target(&device, w, h);
    gpu.run(&device, &queue, &source, &k, &target);
    let got = read_back(&device, &queue, &target, w, h).expect("read back");
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

/// The GPU count against `histogram::measure` on the same picture, sampled down to its copy.
#[test]
fn the_histogram_kernel_counts_what_its_cpu_reference_counts() {
    let Ok(oni_graph::Gpu { device, queue, .. }) = oni_graph::gpu() else {
        eprintln!("no adapter: the histogram kernel is not exercised");
        return;
    };
    let (w, h) = (1300u32, 700u32);
    // Ramps past both ends and a green no sRGB screen shows, so the clip and the end bins are read.
    let pixels: Vec<[f32; 4]> =
        (0..w * h).map(|i| [(i % w) as f32 / w as f32 * 1.2 - 0.1, 0.9 * (i / w) as f32 / h as f32, 0.4, 1.0]).collect();
    let gpu = Gpu::new(&device);
    let picture = upload(&device, &queue, &pixels, w, h).create_view(&Default::default());
    assert!(gpu.count(&device, &queue, &picture, (w, h), Screen::Srgb, false, || {}));
    assert!(!gpu.count(&device, &queue, &picture, (w, h), Screen::Srgb, false, || {}), "a second count waits for the first to be read");
    let counted = landed(&device, &gpu);
    let close = |a: &oni_measure::histogram::Histogram, b: &oni_measure::histogram::Histogram| {
        let total: f32 = b.luma.iter().sum();
        let apart: f32 = a.rgb.iter().zip(&b.rgb).map(|(x, y)| (0..3).map(|c| (x[c] - y[c]).abs()).sum::<f32>()).sum();
        apart / total
    };
    let want = crate::histogram::measure(&pixels, w as usize, h as usize, Screen::Srgb);
    assert_eq!(counted.luma.iter().sum::<f32>(), (640 * 345) as f32, "the copy is 640 by 345");
    assert!(close(&counted, &want) < 2e-3, "{}", close(&counted, &want));
    // Twin: the same picture counted for a P3 screen clips less and is another histogram.
    let p3 = crate::histogram::measure(&pixels, w as usize, h as usize, Screen::DisplayP3);
    assert!(close(&counted, &p3) > 1e-2, "{}", close(&counted, &p3));
}

/// A sheet's clear gaps hold no picture: counted on a sheet they are left out, as the CPU reference
/// leaves them; a frame's own clear corners still count black off a sheet, as Swift's do.
#[test]
fn a_sheet_s_clear_gaps_are_not_counted_as_black() {
    let Ok(oni_graph::Gpu { device, queue, .. }) = oni_graph::gpu() else {
        eprintln!("no adapter: the sheet's count is not exercised");
        return;
    };
    let (w, h) = (300u32, 200u32);
    // Two pictures of a mid grey side by side, a clear band of 40 columns between them.
    let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| if (130..170).contains(&(i % w)) { [0.0; 4] } else { [0.4, 0.4, 0.4, 1.0] }).collect();
    let picture = upload(&device, &queue, &pixels, w, h).create_view(&Default::default());
    let counted = |sheet: bool| {
        let gpu = Gpu::new(&device);
        assert!(gpu.count(&device, &queue, &picture, (w, h), Screen::Srgb, sheet, || {}));
        landed(&device, &gpu)
    };
    let sheet = counted(true);
    let want = crate::histogram::measure_sheet(&pixels, w as usize, h as usize, Screen::Srgb);
    assert_eq!(sheet.luma, want.luma, "the kernel counts what its reference counts");
    assert_eq!(sheet.luma[0], 0.0, "no black where nothing lies");
    assert_eq!(sheet.luma.iter().sum::<f32>(), (260 * 200) as f32, "the pictures alone");
    // Twin: the same picture off a sheet counts its 40 clear columns black.
    let frame = counted(false);
    assert_eq!(frame.luma[0], (40 * 200) as f32);
    assert_eq!(frame.luma, crate::histogram::measure(&pixels, w as usize, h as usize, Screen::Srgb).luma);
}

/// `pixels` laid in an rgba32float texture the kernel reads, the checks' input.
pub fn upload(device: &wgpu::Device, queue: &wgpu::Queue, pixels: &[[f32; 4]], width: u32, height: u32) -> wgpu::Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("linear"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        bytemuck::cast_slice(pixels),
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 16), rows_per_image: None },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    texture
}

/// The count read back once its map has landed: under load the map's callback can trail the poll.
fn landed(device: &wgpu::Device, gpu: &Gpu) -> oni_measure::histogram::Histogram {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        if let Some(h) = gpu.histogram() {
            return h;
        }
        assert!(std::time::Instant::now() < deadline, "the count never landed");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Colours as CIELAB through a profile, the way a colour-managed viewer or the compositor reads them.
fn seen(bytes: &[[u8; 3]], icc: &[u8]) -> Vec<[f64; 3]> {
    let from = lcms2::Profile::new_icc(icc).unwrap();
    let lab = lcms2::Profile::new_lab4_context(lcms2::GlobalContext::new(), &lcms2::white_point_from_temp(5003.0).unwrap()).unwrap();
    let t: lcms2::Transform<[u8; 3], [f64; 3]> =
        lcms2::Transform::new(&from, lcms2::PixelFormat::RGB_8, &lab, lcms2::PixelFormat::Lab_DBL, lcms2::Intent::RelativeColorimetric).unwrap();
    let mut out = vec![[0.0; 3]; bytes.len()];
    t.transform_pixels(bytes, &mut out);
    out
}

/// The profile a drawable tagged for `screen` is matched through.
fn tag(screen: Screen) -> Vec<u8> {
    match screen {
        Screen::Srgb => oni_colour::Space::Srgb,
        Screen::DisplayP3 => oni_colour::Space::DisplayP3,
    }
    .icc_data()
    .unwrap()
}

/// The largest CIE76 gap between the default export, read through its embedded profile, and the canvas
/// encoded for `encoded_for` and read through `shown_as`.
fn export_against_canvas(encoded_for: Screen, shown_as: &[u8]) -> f64 {
    // Skin, sky and a saturated red, linear Rec. 2020.
    let patches = [[0.42f32, 0.27, 0.19, 1.0], [0.14, 0.24, 0.48, 1.0], [0.45, 0.06, 0.04, 1.0]];
    let export = oni_export::Settings::default();
    let image = oni_export::Image { width: 3, height: 1, pixels: patches.concat() };
    let file = oni_export::converted(&image, &export).unwrap();
    let oni_colour::Quantised::Eight(written) = &file.pixels else { panic!("8-bit export") };
    let written: Vec<[u8; 3]> = written.chunks(4).map(|p| [p[0], p[1], p[2]]).collect();
    let k = Params { dither: 0.0, ..Params::new(encoded_for, 3, 1, 0) };
    let canvas: Vec<[u8; 3]> = encode(&patches, &k).chunks(4).map(|p| [p[0], p[1], p[2]]).collect();
    let (a, b) = (seen(&written, &file.icc), seen(&canvas, shown_as));
    a.iter().zip(&b).map(|(x, y)| (0..3).map(|c| (x[c] - y[c]).powi(2)).sum::<f64>().sqrt()).fold(0.0, f64::max)
}

#[test]
fn the_canvas_on_its_tagged_drawable_shows_the_exported_colours() {
    assert_eq!(oni_export::Settings::default().space(), &oni_colour::Space::DisplayP3);
    let gap = export_against_canvas(WINDOW, &tag(WINDOW));
    assert!(gap <= 2.0, "the canvas sits {gap:.2} ΔE from the export");
    // Twin: sRGB numbers on an untagged layer, which a Display P3 panel shows as its own, saturate.
    let stretched = export_against_canvas(Screen::Srgb, &tag(Screen::DisplayP3));
    assert!(stretched > 4.0, "the stretched canvas sits only {stretched:.2} ΔE from the export");
}
