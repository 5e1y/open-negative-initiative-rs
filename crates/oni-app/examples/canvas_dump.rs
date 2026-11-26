//! `canvas_dump <photo> <out>`: the canvas's own renders of a photo, at fit and at 100 % about its centre, on a
//! 3456 × 2034 canvas once the whole decode is in, each as `w h` then f32 RGBA, linear.

use oni_app::render::{self, Device, Engine, Reading, View};
use oni_graph::{Image, Rect};
use oni_settings::sidecar;
use std::io::Write;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (photo, out) = (Path::new(&args[0]), Path::new(&args[1]));
    std::fs::create_dir_all(out).unwrap();
    let s = sidecar::read(photo, true).expect("a sidecar").settings;
    let device = Device::open().expect("a GPU");
    let engine = Engine::shared(&device);
    let mut e = engine.lock().unwrap();
    let negative = e.prepare(render::decode(photo, Reading::of(&s)).expect("a decode")).unwrap();
    let full = e.full(render::decode_full(photo, Reading::of(&s)).expect("a decode")).unwrap();
    for (name, scale) in [("fit", None), ("100", Some(1.0))] {
        let view = View { viewport: oni_app::viewport::Viewport { scale, offset: (0.0, 0.0) }, size: (3456.0, 2034.0), full: Some(full.clone()), coarse: false };
        let (source, _) = e.resident(&negative, &s, Some(&view)).unwrap();
        let r = source.extent();
        let pixels = e.parts().0.render(&Image::new(&source), Rect::new(r.x, r.y, r.w, r.h), None).unwrap();
        let mut f = std::io::BufWriter::new(std::fs::File::create(out.join(format!("{name}.f32"))).unwrap());
        writeln!(f, "{} {}", r.w, r.h).unwrap();
        for p in &pixels {
            for v in p {
                f.write_all(&v.to_le_bytes()).unwrap();
            }
        }
        println!("{name}: {} x {}", r.w, r.h);
    }
}
