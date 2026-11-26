//! `welcome_stage <out.png> <gift|message> [seconds since opened] [width height]`: the stage as the GPU draws it;
//! `--stills <dir>`: each gift's still, wrapped and opened. `ONI_STAGE_WGSL`: the shader's folder; `ONI_STAGE_TIME`: a pace.

use oni_app::render::Device;
use oni_app::stage::direction::{Cue, frame};
use oni_app::stage::{FINISH, Stage, source, unwrap};
use std::path::Path;
use std::time::Instant;

/// A still's size: the stage's own shape, as wide as a large window's middle needs.
const STILL: (u32, u32) = (1600, 1000);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let gpu = Device::open().expect("a GPU");
    let (shader, finish) = match std::env::var("ONI_STAGE_WGSL") {
        Ok(folder) => {
            let read = |part: &str| std::fs::read_to_string(format!("{folder}/{part}.wgsl")).unwrap();
            (["scene", "light", "picture"].map(read).join("\n"), read("finish"))
        }
        Err(_) => (source(), FINISH.to_string()),
    };
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut stage = Stage::with_source(&gpu.device, &shader, &finish);
    if let Some(error) = pollster::block_on(scope.pop()) {
        panic!("{error}");
    }
    if args[0] == "--stills" {
        let out = Path::new(&args[1]);
        for gift in 0..4 {
            for (name, since) in [("closed", None), ("opened", Some(unwrap::length(gift) + 30.0))] {
                let mut cue = Cue::at(Some(gift), since);
                cue.aspect = STILL.0 as f32 / STILL.1 as f32;
                let pixels = drawn(&gpu, &mut stage, &cue, STILL);
                jpeg(&out.join(format!("gift-{gift}-{name}.jpg")), &pixels, STILL);
            }
        }
        return;
    }
    let number = |i: usize, rest: f32| args.get(i).map_or(rest, |v| v.parse().unwrap());
    let gift = args[1].parse::<usize>().ok();
    let since = args.get(2).map(|v| v.parse::<f32>().unwrap()).filter(|s| *s >= 0.0);
    let (width, height) = (number(3, 1728.0) as u32, number(4, 1080.0) as u32);
    let mut cue = Cue::at(gift, since);
    cue.aspect = width as f32 / height as f32;
    if let Some(count) = std::env::var("ONI_STAGE_TIME").ok().and_then(|v| v.parse::<u32>().ok()) {
        let target = Stage::target(&gpu.device, width, height);
        let began = Instant::now();
        for _ in 0..count {
            cue.clock += 0.008;
            stage.draw(&gpu.device, &gpu.queue, &frame(&cue), &target);
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        }
        println!("{:.2} ms a picture", began.elapsed().as_secs_f64() * 1000.0 / f64::from(count));
    }
    let pixels = drawn(&gpu, &mut stage, &cue, (width, height));
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&args[0]).unwrap()), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
}

fn drawn(gpu: &Device, stage: &mut Stage, cue: &Cue, (width, height): (u32, u32)) -> Vec<u8> {
    let target = Stage::target(&gpu.device, width, height);
    stage.draw(&gpu.device, &gpu.queue, &frame(cue), &target);
    oni_app::display::read_back(&gpu.device, &gpu.queue, &target, width, height).expect("the picture")
}

fn jpeg(path: &Path, rgba: &[u8], (width, height): (u32, u32)) {
    let encoder = jpeg_encoder::Encoder::new_file(path, 88).unwrap();
    encoder.encode(rgba, width as u16, height as u16, jpeg_encoder::ColorType::Rgba).unwrap();
    println!("{}", path.display());
}
