//! `open_cost <photo> <stock> [<another photo>]`: the time each step of opening a digital photo on `stock` takes, as the editor opens it:
//! LibRaw's half-size read, the preview laid on the GPU, the preview's frozen scan, the first render at fit, then the
//! full definition's read and its scan for 100 %; twice, the decode cache cold then warm. `ONI_CACHES` names the cache.

use oni_app::render::{self, Device, Engine, Reading, View};
use oni_graph::{Image, Rect};
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock};
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let photo = Path::new(&args[0]);
    let stock = VirtualStock::of(Box::leak(args[1].clone().into_boxed_str()));
    let s = PipelineSettings::rest(ConversionMode::Digital, true).picking(stock);
    let device = Device::open().expect("a GPU");
    // The app's own decode cache, under `ONI_CACHES`.
    oni_decode::cache::keep_in(oni_app::system::caches().join("Decodes"), 8 << 30);
    // The kernels compiled as the app compiles them at launch, before any photo opens.
    let began = Instant::now();
    Engine::shared(&device).prewarm();
    println!("kernels compiled at launch             {:>7.1} ms", began.elapsed().as_secs_f64() * 1e3);
    // A double click on a photo not yet decoded: its first press selects it and starts the decode ahead, the second
    // opens it `DOUBLE_CLICK` later and waits for that decode; the time from the second press to the image.
    if let Some(selected) = args.get(2).map(Path::new) {
        const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(250);
        let ahead = selected.to_path_buf();
        let reading = Reading::of(&s);
        let started = std::thread::spawn(move || { let _ = render::decode(&ahead, reading); });
        std::thread::sleep(DOUBLE_CLICK);
        let opened = Instant::now();
        let engine = Engine::shared(&device);
        let mut e = engine.lock().unwrap();
        let negative = e.prepare(render::decode(selected, Reading::of(&s)).expect("a decode")).unwrap();
        let fit = View { viewport: oni_app::viewport::Viewport { scale: None, offset: (0.0, 0.0) }, size: (3456.0, 2034.0), full: None, coarse: false };
        let (shown, _) = e.resident(&negative, &s, Some(&fit)).unwrap();
        let r = shown.extent();
        e.parts().0.render(&Image::new(&shown), Rect::new(r.x, r.y, 1, 1), None).unwrap();
        println!("selected then opened {} ms later: second press to image {:>7.1} ms", DOUBLE_CLICK.as_millis(), opened.elapsed().as_secs_f64() * 1e3);
        started.join().unwrap();
    }
    for pass in ["cold", "warm"] {
        let engine = Engine::shared(&device);
        let mut e = engine.lock().unwrap();
        let mut at = Instant::now();
        let mut step = |name: &str, at: &mut Instant| {
            println!("{pass:>4} {name:<34} {:>7.1} ms", at.elapsed().as_secs_f64() * 1e3);
            *at = Instant::now();
        };
        let decoded = render::decode(photo, Reading::of(&s)).expect("a decode");
        step("half-size read (LibRaw or cache)", &mut at);
        let negative = e.prepare(decoded).unwrap();
        step("preview laid on the GPU", &mut at);
        let _ = oni_pipeline::film::stock(stock);
        step("film baked (once a process)", &mut at);
        let sync = |e: &mut Engine, src: &oni_graph::Source| {
            let r = src.extent();
            e.parts().0.render(&Image::new(src), Rect::new(r.x, r.y, 1, 1), None).unwrap();
        };
        let fit = View { viewport: oni_app::viewport::Viewport { scale: None, offset: (0.0, 0.0) }, size: (3456.0, 2034.0), full: None, coarse: false };
        let (shown, _) = e.resident(&negative, &s, Some(&fit)).unwrap();
        sync(&mut e, &shown);
        step("scan baked and first render at fit", &mut at);
        let (again, _) = e.resident(&negative, &s, Some(&fit)).unwrap();
        sync(&mut e, &again);
        step("a second render at fit, scan kept", &mut at);
        // A notch at fit: made on the hand's press, then made ahead and rendered on its press.
        let grain = oni_app::fields::Field::GrainAmount;
        let notch = |k: f32| grain.knob().set(&s, k);
        let (pressed, _) = e.resident(&negative, &notch(0.1719), Some(&fit)).unwrap();
        sync(&mut e, &pressed);
        step("a grain notch made on its press", &mut at);
        e.preload(&negative, &notch(0.3783)).unwrap();
        let (src, _) = e.resident(&negative, &s, Some(&fit)).unwrap();
        sync(&mut e, &src);
        step("a neighbour's scan made ahead", &mut at);
        let (ahead, _) = e.resident(&negative, &notch(0.3783), Some(&fit)).unwrap();
        sync(&mut e, &ahead);
        step("the neighbour's notch, made ahead", &mut at);
        let exposure = oni_app::fields::Field::FilmExposure;
        e.preload(&negative, &exposure.knob().set(&s, 1.0 / 3.0)).unwrap();
        let (src, _) = e.resident(&negative, &s, Some(&fit)).unwrap();
        sync(&mut e, &src);
        step("an exposure third's scan made ahead", &mut at);
        let full = render::decode_full(photo, Reading::of(&s)).expect("a decode");
        step("full definition read", &mut at);
        let full = e.full(full).unwrap();
        step("full definition laid on the GPU", &mut at);
        let hundred = View { viewport: oni_app::viewport::Viewport { scale: Some(1.0), offset: (0.0, 0.0) }, size: (3456.0, 2034.0), full: Some(full.clone()), coarse: false };
        let (sharp, _) = e.resident(&negative, &s, Some(&hundred)).unwrap();
        sync(&mut e, &sharp);
        step("full scan baked and render at 100 %", &mut at);
    }
}
