use oni_pipeline::look_lab::edits;
use oni_pipeline::{conversion, oni_look, scanner::{self, MACHINE}};
use oni_render::{ScannerEncode, ScannerOutput, cpu};
use oni_settings::modes::{Face, ScannerMachine};
use std::path::Path;
fn q(v: &mut Vec<f32>, f: f64) -> f32 { v.sort_by(f32::total_cmp); v[((v.len() - 1) as f64 * f) as usize] }
fn line(name: &str, v: &mut Vec<f32>) { println!("{name:<44} p1 {:7.3}  p50 {:7.3}  p99 {:7.3}  p99.9 {:7.3}  >=0.995: {:5.2} %", q(v, 0.01), q(v, 0.5), q(v, 0.99), q(v, 0.999), v.iter().filter(|x| **x >= 0.995).count() as f64 * 100.0 / v.len() as f64); }
fn main() {
    let raw = Path::new(&std::env::args().nth(1).unwrap());
    let base = oni_settings::sidecar::read(raw, false, false).unwrap().settings;
    let l = oni_app::render::decode(raw, oni_app::render::Reading::of(&base)).unwrap().linear;
    let gain = (-base.capture_stops).exp2();
    let px: Vec<[f32; 4]> = l.pixels.iter().step_by(5).map(|p| [p[0] * gain, p[1] * gain, p[2] * gain, 1.0]).collect();
    let g = base.levels.green;
    println!("window green {:.3}..{:.3} (span {:.3}); ceiling {:.3} -> pedestal {:.4}; print exposure {}; gains {:?}", g.black, g.white, g.white - g.black, base.density_ceiling, 10f32.powf(-base.density_ceiling), base.print.exposure, base.gains.stops);
    // ONI machine
    let mut oni = base.clone(); (oni.face, oni.scanner_machine) = (Face::Scanner, ScannerMachine::Oni);
    let os = scanner::on_oni_machine(&oni).unwrap();
    let k47 = conversion::stage47(&os);
    let mut a: Vec<f32> = Vec::new(); let mut b = Vec::new(); let mut c = Vec::new();
    let out = ScannerOutput { warmth_share: 0.0, lift: scanner::LIFT, contrast: 0.0 };
    for p in &px {
        let lin = cpu::stage47(*p, &k47);
        a.push(lin[1]);
        let code = cpu::scanner_encode(lin, &ScannerEncode { root: oni_look::ROOT });
        b.push(code[1]);
        let looked = cpu::color_cube(code, &oni_look::CUBE);
        let o = cpu::scanner_encode(cpu::scanner_output(looked, looked, &out), &ScannerEncode { root: 1.0 });
        c.push(o[1]);
    }
    println!("== ONI machine (print exposure forced to {}, Pro conversion at rest)", os.print.exposure);
    line("1 stage47 linear (window + print curve)", &mut a);
    line("2 encoded for its look (root 2)", &mut b);
    line("3 after ONI look cube -> display code", &mut c);
    // Frontier
    let mut fr = base.clone(); (fr.face, fr.scanner_machine) = (Face::Scanner, ScannerMachine::Frontier);
    let k = scanner::density(&fr);
    let table = edits::table(&fr.frontier_lab);
    println!("== Frontier: median {:.3}, Pro black share {:.3}, scale {:.3} (fit's would be {:.3}), table window x {:.3}..{:.3}", k.median[1], scanner::pro_black(&fr), k.scale, 1.3 / (g.white - g.black), scanner::ENDS.0, scanner::ENDS.1);
    let (mut d, mut x, mut u, mut o2) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for p in &px {
        let t = (p[1] + k.pedestal) * k.gain[1];
        d.push(-t.max(k.log_guard).log10());
        let n = cpu::scanner_density(*p, &k);
        let xg = n[1] * MACHINE.span() + MACHINE.low();
        x.push(xg);
        u.push(((xg - scanner::ENDS.0) / (scanner::ENDS.1 - scanner::ENDS.0)).clamp(0.0, 1.0));
        let cc = cpu::color_cube(n, &table);
        o2.push(cpu::scanner_encode(cpu::scanner_output(cc, cc, &out), &ScannerEncode { root: 1.0 })[1]);
    }
    line("1 green density (pedestal + gains)", &mut d);
    line("2 x = scale*(d - median) + neutral", &mut x);
    line("3 straight paper u (table window, clamped)", &mut u);
    line("4 after Lab table -> display code", &mut o2);
}
