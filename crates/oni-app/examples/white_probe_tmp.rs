use oni_pipeline::look_lab::edits;
use oni_pipeline::scanner::{self, MACHINE};
use oni_render::cpu;
use oni_settings::frontier_lab::FrontierLab;
use oni_settings::modes::{Face, ScannerMachine};
use std::path::Path;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let raw = Path::new(&a[1]);
    let mut s = oni_settings::sidecar::read(raw, true, false).unwrap().settings;
    (s.face, s.scanner_machine) = (Face::Scanner, ScannerMachine::Frontier);
    let l = oni_app::render::decode(raw, oni_app::render::Reading::of(&s)).unwrap().linear;
    let k = scanner::density(&s);
    let table = edits::table(&FrontierLab::default());
    let gain = (-s.capture_stops).exp2();
    let (mut xs, mut outs) = (Vec::new(), Vec::new());
    for p in l.pixels.iter().step_by(5) {
        let n = cpu::scanner_density([p[0] * gain, p[1] * gain, p[2] * gain, 1.0], &k);
        xs.push(n[1] * MACHINE.span() + MACHINE.low());
        outs.push(cpu::color_cube(n, &table)[1]);
    }
    let burnt = outs.iter().filter(|v| **v >= 0.995).count() as f64 / outs.len() as f64;
    xs.sort_by(f32::total_cmp);
    let q = |f: f64| xs[((xs.len() - 1) as f64 * f) as usize];
    println!("table window {:.3}..{:.3}, scale {:.3}; image x p1 {:.3} p50 {:.3} p99 {:.3} p99.9 {:.3}; burnt {:.2} %", scanner::ENDS.0, scanner::ENDS.1, k.scale, q(0.01), q(0.5), q(0.99), q(0.999), burnt * 100.0);
}
