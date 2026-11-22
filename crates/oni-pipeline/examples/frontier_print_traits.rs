//! `frontier_print_traits`: a scene's blues and skins on the virtual Portra 800, printed at rest on the Frontier paper,
//! today's Scanner-face Frontier, Portra Endura and ONI's print: Oklab hue and chroma against the scene's.

use oni_pipeline::look_lab::measure::{lch, linear_rgb, oklab, turn};
use oni_pipeline::{conversion, film, frontier_look, scanner};
use oni_render::{ScannerEncode, ScannerOutput, cpu};
use oni_settings::modes::ScannerMachine;
use oni_settings::{ConversionMode, Face, PaperStock, PipelineSettings, VirtualStock};

type V3 = [f64; 3];

const TO2020: [V3; 3] = [[0.627_404, 0.329_283, 0.043_313], [0.069_097, 0.919_540, 0.011_362], [0.016_391, 0.088_013, 0.895_595]];
const TO709: [V3; 3] = [[1.660_49, -0.587_641, -0.072_850], [-0.124_550, 1.132_9, -0.008_349], [-0.018_150, -0.100_579, 1.118_73]];

fn apply(m: &[V3; 3], v: V3) -> V3 {
    std::array::from_fn(|r| (0..3).map(|c| m[r][c] * v[c]).sum())
}

/// A scene colour of Oklab lightness, chroma and hue, as linear Rec. 2020.
fn scene(l: f64, c: f64, h: f64) -> V3 {
    let h = h.to_radians();
    apply(&TO2020, linear_rgb([l, c * h.cos(), c * h.sin()]))
}

/// The print of `colour` on settings `s`, in Oklab, as the display clips it.
fn printed(colour: V3, s: &PipelineSettings) -> V3 {
    let stock = film::stock(VirtualStock::KodakPortra800);
    let lit = conversion::scene_white(s).map_or([colour[0] as f32, colour[1] as f32, colour[2] as f32, 1.0], |m| {
        cpu::color_matrix([colour[0] as f32, colour[1] as f32, colour[2] as f32, 1.0], &m)
    });
    let negative = cpu::virtual_negative(lit, &stock.kernel, &stock.table);
    let machine = scanner::on_oni_machine(s);
    let k = conversion::stage47(machine.as_ref().unwrap_or(s));
    let mut out = cpu::stage47(negative, &k);
    if machine.is_some() && s.paper().is_none() {
        let code = cpu::scanner_encode(out, &ScannerEncode { root: 1.0 });
        let looked = cpu::color_cube(code, &frontier_look::CUBE);
        out = cpu::scanner_output(looked, looked, &ScannerOutput { warmth_share: 0.0, lift: scanner::LIFT, contrast: 0.0 });
    }
    oklab(apply(&TO709, [out[0], out[1], out[2]].map(f64::from)).map(|v| v.clamp(0.0, 1.0)))
}

fn prints() -> Vec<(&'static str, PipelineSettings)> {
    let digital = PipelineSettings::rest(ConversionMode::Digital, true);
    let on = |p: Option<PaperStock>| PipelineSettings { print_paper: p, ..digital.clone() };
    let mut today = on(None);
    (today.face, today.scanner_machine) = (Face::Scanner, ScannerMachine::Frontier);
    vec![
        ("Frontier SP-3000 paper", on(Some(PaperStock::FujifilmFrontierSp3000))),
        ("today's Scanner-face Frontier", today),
        ("Portra Endura", on(Some(PaperStock::KodakPortraEndura))),
        ("ONI's print", on(None)),
    ]
}

fn main() {
    let blues: Vec<V3> = (0..8).flat_map(|h| (0..4).flat_map(move |l| (0..3).map(move |c| [0.4 + 0.1 * l as f64, 0.06 + 0.03 * c as f64, 230.0 + 5.0 * h as f64]))).collect();
    let skins: Vec<V3> = (0..8).flat_map(|h| (0..4).flat_map(move |l| (0..3).map(move |c| [0.5 + 0.1 * l as f64, 0.04 + 0.03 * c as f64, 40.0 + 5.0 * h as f64]))).collect();
    let grey = |s: &PipelineSettings| lch(printed(scene(0.6, 0.0, 0.0), s));
    println!("{} blues (h 230-265°, L 0.4-0.7, C 0.06-0.12), {} skins (h 40-75°, L 0.5-0.8, C 0.04-0.10)", blues.len(), skins.len());
    for (name, s) in prints() {
        let family = |set: &[V3]| {
            let out: Vec<(V3, V3)> = set.iter().map(|&[l, c, h]| ([l, c, h], lch(printed(scene(l, c, h), &s)))).collect();
            let n = out.len() as f64;
            let dh = out.iter().map(|(i, o)| turn(o[2], i[2])).sum::<f64>() / n;
            let dc = out.iter().map(|(i, o)| o[1] / i[1]).sum::<f64>() / n;
            (out, dh, dc)
        };
        let (b, bh, bc) = family(&blues);
        let violet = b.iter().filter(|(_, o)| o[2] > 270.0).count() as f64 / b.len() as f64;
        let (k, kh, kc) = family(&skins);
        let red = k.iter().map(|(i, o)| (o[1] * o[2].to_radians().cos()) / (i[1] * i[2].to_radians().cos())).sum::<f64>() / k.len() as f64;
        let one = |l: f64, c: f64, h: f64| lch(printed(scene(l, c, h), &s));
        let (sky, skin) = (one(0.6, 0.1, 247.0), one(0.65, 0.068, 58.0));
        let g = grey(&s);
        println!("{name}: blues Δh {bh:+.1}° chroma ×{bc:.2}, {:.0} % past 270°; skins Δh {kh:+.1}° chroma ×{kc:.2}, red a ×{red:.2}", 100.0 * violet);
        println!("  247° blue → {:.1}° C {:.3}; 58° skin → {:.1}° C {:.3}; grey L 0.6 → L {:.3} C {:.4} at {:.0}°", sky[2], sky[1], skin[2], skin[1], g[0], g[1], g[2]);
    }
}
