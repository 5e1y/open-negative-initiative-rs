//! `welcome_stage <out.png> <act> <t> [width height [step step_t]]`: one picture of the welcome's stage as the GPU
//! draws it. Acts: overture, hall, unwrap0..3, page0..3, message; `ONI_STAGE_WGSL` names a folder to read the shader from.

use oni_app::render::Device;
use oni_app::stage::direction::{Act, Cue, frame};
use oni_app::stage::{FINISH, Stage, source};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let number = |i: usize, rest: f32| args.get(i).map_or(rest, |v| v.parse().unwrap());
    let (width, height) = (number(3, 1728.0) as u32, number(4, 1080.0) as u32);
    let index = |name: &str| name[name.len() - 1..].parse::<usize>().unwrap();
    let act = match args[1].as_str() {
        "overture" => Act::Overture,
        "hall" => Act::Hall,
        "message" => Act::Message,
        name if name.starts_with("unwrap") => Act::Unwrap(index(name)),
        name => Act::Page(index(name)),
    };
    let mut cue = Cue::at(act, number(2, 0.0));
    cue.aspect = width as f32 / height as f32;
    cue.step = number(5, 0.0) as usize;
    cue.step_t = number(6, 0.0);
    if let Act::Page(_) = act {
        cue.words = 420.0 / 800.0;
    }
    if let Ok(opened) = std::env::var("ONI_STAGE_OPENED") {
        cue.opened = std::array::from_fn(|i| opened.as_bytes().get(i) == Some(&b'1'));
    }
    let gpu = Device::open().expect("a GPU");
    let (shader, finish) = match std::env::var("ONI_STAGE_WGSL") {
        Ok(folder) => {
            let read = |part: &str| std::fs::read_to_string(format!("{folder}/{part}.wgsl")).unwrap();
            (["scene", "light", "plate", "picture"].map(read).join("\n"), read("finish"))
        }
        Err(_) => (source(), FINISH.to_string()),
    };
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut stage = Stage::with_source(&gpu.device, &gpu.queue, &shader, &finish);
    if let Some(error) = pollster::block_on(scope.pop()) {
        panic!("{error}");
    }
    let target = Stage::target(&gpu.device, width, height);
    // `ONI_STAGE_TIME`: that many pictures drawn one after the other, each awaited, for the GPU's own pace.
    if let Some(count) = std::env::var("ONI_STAGE_TIME").ok().and_then(|v| v.parse::<u32>().ok()) {
        let began = Instant::now();
        for i in 0..count {
            cue.clock += 0.008;
            cue.t += 0.008 * (i % 2) as f32;
            stage.draw(&gpu.device, &gpu.queue, &frame(&cue), &target);
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        }
        println!("{:.2} ms a picture", began.elapsed().as_secs_f64() * 1000.0 / f64::from(count));
    }
    let began = Instant::now();
    stage.draw(&gpu.device, &gpu.queue, &frame(&cue), &target);
    let pixels = oni_app::display::read_back(&gpu.device, &gpu.queue, &target, width, height).expect("the picture");
    println!("{:?} at {:.2} s: {width} x {height} in {:.1} ms", act, cue.t, began.elapsed().as_secs_f64() * 1000.0);
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&args[0]).unwrap()), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
}
