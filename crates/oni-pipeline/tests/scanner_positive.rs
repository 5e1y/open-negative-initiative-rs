//! A slide on the Scanner face lands near its Pro render, as a negative does, and a negative on the
//! Scanner face renders as before, bit for bit.

use oni_graph::{Graph, Image, Rect};
use oni_pipeline::{Inputs, Pipeline, scanner};
use oni_settings::modes::{Face, ScannerMachine};
use oni_settings::scanner::ScannerClicks;
use oni_settings::{ConversionMode, PipelineSettings};

const W: u32 = 128;
const H: u32 = 4;

/// A scene of known linear tones: a grey ramp over nine stops, then sky, skin and foliage along it.
fn scene() -> Vec<[f32; 4]> {
    let tints = [[1.0, 1.0, 1.0], [0.55, 0.75, 1.0], [1.0, 0.72, 0.55], [0.45, 0.8, 0.35]];
    (0..H as usize)
        .flat_map(|row| (0..W).map(move |x| (row, x)))
        .map(|(row, x)| {
            let level = 0.9 * (-9.0 * (1.0 - x as f32 / (W - 1) as f32)).exp2();
            let t = tints[row];
            [level * t[0], level * t[1], level * t[2], 1.0]
        })
        .collect()
}

/// The machine's settings as the Scanner face built them before slides entered the look under their rest.
fn former(frame: &PipelineSettings) -> PipelineSettings {
    let mut out = scanner::on_oni_machine(frame).expect("the Scanner face");
    out.print.exposure = 0.0;
    out
}

struct Bench {
    g: Graph,
    source: oni_graph::Source,
}

impl Bench {
    fn render(&mut self, pixels: Option<&[[f32; 4]]>, frame: &PipelineSettings, machine: Option<&PipelineSettings>) -> Vec<[f32; 4]> {
        let source = match pixels {
            Some(p) => self.g.upload(W, H, p).expect("the scene uploads"),
            None => self.source.clone(),
        };
        let image = Pipeline::new()
            .chain(&mut self.g, Image::new(&source), frame, machine, (1.0, f64::from(W)), false, &Inputs::default())
            .expect("the recipe");
        self.g.render(&image, Rect { x: 0, y: 0, w: W, h: H }, None).expect("the scene renders")
    }
}

fn mean_luminance(px: &[[f32; 4]]) -> f64 {
    px.iter().map(|p| f64::from(0.2627 * p[0] + 0.6780 * p[1] + 0.0593 * p[2])).sum::<f64>() / px.len() as f64
}

fn clipped(px: &[[f32; 4]]) -> f64 {
    px.iter().filter(|p| p[..3].iter().any(|c| *c >= 0.999)).count() as f64 / px.len() as f64
}

fn at_rest(mode: ConversionMode, machine: ScannerMachine) -> PipelineSettings {
    PipelineSettings { face: Face::Scanner, scanner_machine: machine, ..PipelineSettings::rest(mode, true).per_pixel_only() }
}

/// Within half a stop of Pro's mean, no more clipped; the looks' own lift on a negative is about 0.4 stop.
const MARGIN_STOPS: f64 = 0.5;

#[test]
fn a_slide_on_the_scanner_face_lands_near_its_pro_render() {
    let Ok(mut g) = Graph::new() else { return println!("SKIPPED: no GPU") };
    let source = g.upload(W, H, &scene()).expect("the scene uploads");
    let mut bench = Bench { g, source };
    let pro_frame = PipelineSettings::rest(ConversionMode::Positive, true).per_pixel_only();
    let pro = bench.render(None, &pro_frame, None);
    for machine in [ScannerMachine::Oni, ScannerMachine::Frontier] {
        let frame = at_rest(ConversionMode::Positive, machine);
        let scanned = bench.render(None, &frame, scanner::on_oni_machine(&frame).as_ref());
        let stops = (mean_luminance(&scanned) / mean_luminance(&pro)).log2();
        assert!(stops.abs() <= MARGIN_STOPS, "{machine:?}: {stops:+.2} stops from Pro");
        assert!(clipped(&scanned) <= clipped(&pro), "{machine:?}: {} clipped against {}", clipped(&scanned), clipped(&pro));
        // Twin: the former chain, the slide entering the look at its own rest, fails the same margin.
        let before = bench.render(None, &frame, Some(&former(&frame)));
        let over = (mean_luminance(&before) / mean_luminance(&pro)).log2();
        assert!(over > MARGIN_STOPS, "{machine:?}: the former chain sat {over:+.2} stops from Pro");
        println!("{machine:?}: {stops:+.2} stops from Pro, {over:+.2} before");
    }
}

#[test]
fn a_negative_on_the_scanner_face_renders_as_before_bit_for_bit() {
    let Ok(mut g) = Graph::new() else { return println!("SKIPPED: no GPU") };
    // The scene as a negative's transmittances: dense where the slide is light.
    let negative: Vec<[f32; 4]> = scene().iter().map(|p| [0.8 - 0.75 * p[0], 0.7 - 0.65 * p[1], 0.6 - 0.55 * p[2], 1.0]).collect();
    let source = g.upload(W, H, &negative).expect("the negative uploads");
    let mut bench = Bench { g, source };
    for mode in [ConversionMode::Negative, ConversionMode::Monochrome] {
        for machine in [ScannerMachine::Oni, ScannerMachine::Frontier] {
            let mut frame = at_rest(mode, machine);
            frame.scanner = ScannerClicks::new(3, -2, 1, 4, 2);
            frame.scanner_finish.contrast = 0.2;
            let now = bench.render(None, &frame, scanner::on_oni_machine(&frame).as_ref());
            let before = bench.render(None, &frame, Some(&former(&frame)));
            assert!(now.iter().flatten().zip(before.iter().flatten()).all(|(a, b)| a.to_bits() == b.to_bits()), "{mode:?} {machine:?} moved");
        }
    }
    // Twin: the comparison sees the change where there is one, on a slide.
    let slide = at_rest(ConversionMode::Positive, ScannerMachine::Oni);
    let pixels = scene();
    let (now, before) = (
        bench.render(Some(&pixels), &slide, scanner::on_oni_machine(&slide).as_ref()),
        bench.render(Some(&pixels), &slide, Some(&former(&slide))),
    );
    assert!(now.iter().flatten().zip(before.iter().flatten()).any(|(a, b)| a.to_bits() != b.to_bits()));
}
