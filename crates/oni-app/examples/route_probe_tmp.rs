use oni_pipeline::look_lab::edits;
use oni_pipeline::{conversion, oni_look, scanner};
use oni_render::{ScannerEncode, cpu};
use oni_settings::modes::{Face, ScannerMachine};
use std::path::Path;
fn main() {
    let raw = Path::new(&std::env::args().nth(1).unwrap());
    let mut s = oni_settings::sidecar::read(raw, false, false).unwrap().settings;
    let l = oni_app::render::decode(raw, oni_app::render::Reading::of(&s)).unwrap().linear;
    s.face = Face::Scanner;
    let table = edits::table_on_code(&s.frontier_lab);
    for (name, machine) in [("ONI", ScannerMachine::Oni), ("Frontier", ScannerMachine::Frontier)] {
        s.scanner_machine = machine;
        let k = conversion::stage47(&scanner::on_oni_machine(&s).unwrap());
        let gain = (-s.capture_stops).exp2();
        let mut g: Vec<f32> = l.pixels.iter().step_by(5).map(|p| {
            let lin = cpu::stage47([p[0] * gain, p[1] * gain, p[2] * gain, 1.0], &k);
            if machine == ScannerMachine::Oni { cpu::color_cube(cpu::scanner_encode(lin, &ScannerEncode { root: oni_look::ROOT }), &oni_look::CUBE)[1] }
            else { cpu::color_cube(cpu::scanner_encode(lin, &ScannerEncode { root: 1.0 }), &table)[1] }
        }).collect();
        let clip = g.iter().filter(|v| **v >= 0.995).count() as f64 * 100.0 / g.len() as f64;
        g.sort_by(f32::total_cmp);
        let q = |f: f64| g[((g.len() - 1) as f64 * f) as usize];
        println!("{name:>9}: green code p1 {:.3} p50 {:.3} p95 {:.3} p99 {:.3}, at white {clip:.1} %", q(0.01), q(0.5), q(0.95), q(0.99));
    }
}
