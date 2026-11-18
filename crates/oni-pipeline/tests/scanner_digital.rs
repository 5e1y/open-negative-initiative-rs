//! A digital photo on the Scanner face: the keys are stops of gain on its scene, its As Shot white is
//! the machine's, its levels come before the machine's print and its curves after the look; it lands
//! near its Pro render, and on Pro the face's values render nothing.

mod common;

use common::{V4, graph, judge, mapped, noise, whole};
use oni_graph::{Graph, Image, Source};
use oni_pipeline::{Inputs, Pipeline, Request, conversion, finishing, oni_look, scanner};
use oni_render::{ScannerEncode, ScannerOutput, cpu};
use oni_settings::axis::density_axis;
use oni_settings::cg::Point;
use oni_settings::modes::{Face, ScannerMachine};
use oni_settings::scanner::ScannerClicks;
use oni_settings::white::White;
use oni_settings::{ConversionMode, PipelineSettings};

const W: u32 = 128;
const H: u32 = 4;

/// A scene of known linear tones: a grey ramp over nine stops, then sky, skin and foliage along it.
fn scene() -> Vec<V4> {
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

fn digital(face: Face, machine: ScannerMachine) -> PipelineSettings {
    PipelineSettings { face, scanner_machine: machine, ..PipelineSettings::rest(ConversionMode::Digital, true).per_pixel_only() }
}

fn render(g: &mut Graph, scan: &Source, s: &PipelineSettings, measuring: bool) -> Vec<V4> {
    let request = Request { measuring, ..Request::default() };
    let image = Pipeline::new().image(g, scan, s, &request, &Inputs::default()).expect("the recipe builds");
    whole(g, &image)
}

fn mean_luminance(px: &[V4]) -> f64 {
    px.iter().map(|p| f64::from(0.2627 * p[0] + 0.6780 * p[1] + 0.0593 * p[2])).sum::<f64>() / px.len() as f64
}

fn clipped(px: &[V4]) -> f64 {
    px.iter().filter(|p| p[..3].iter().any(|c| *c >= 0.999)).count() as f64 / px.len() as f64
}

/// Within half a stop of Pro's mean, no more clipped, as a slide's Scanner face lands.
const MARGIN_STOPS: f64 = 0.5;

#[test]
fn a_digital_photo_on_the_scanner_face_lands_near_its_pro_render() {
    let Some(mut g) = graph("a_digital_photo_on_the_scanner_face_lands_near_its_pro_render") else { return };
    let scan = g.upload(W, H, &scene()).unwrap();
    let pro = render(&mut g, &scan, &digital(Face::Pro, ScannerMachine::Oni), false);
    for machine in [ScannerMachine::Oni, ScannerMachine::Frontier] {
        let frame = digital(Face::Scanner, machine);
        let scanned = render(&mut g, &scan, &frame, false);
        let stops = (mean_luminance(&scanned) / mean_luminance(&pro)).log2();
        println!("{machine:?}: {stops:+.3} stops from Pro, {} clipped against {}", clipped(&scanned), clipped(&pro));
        assert!(stops.abs() <= MARGIN_STOPS, "{machine:?}: {stops:+.2} stops from Pro");
        assert!(clipped(&scanned) <= clipped(&pro), "{machine:?}");
        // Twin: the photo entering the look at its own rest, no stop under it, sits beyond the margin.
        let mut at_rest = scanner::on_oni_machine(&frame).unwrap();
        at_rest.print.exposure = 0.0;
        let image = Pipeline::new().chain(&mut g, Image::new(&scan), &frame, Some(&at_rest), (1.0, f64::from(W)), false, &Inputs::default()).unwrap();
        let over = (mean_luminance(&whole(&mut g, &image)) / mean_luminance(&pro)).log2();
        println!("{machine:?}: {over:+.3} stops entering at rest");
        assert!(over > MARGIN_STOPS, "{machine:?}: entering at rest sat {over:+.2} stops from Pro");
    }
}

#[test]
fn the_keys_are_the_photos_only_gains_in_stops_of_its_scene() {
    let mut frame = digital(Face::Scanner, ScannerMachine::Oni);
    frame.scanner = ScannerClicks::new(4, 2, -2, 6, 0);
    frame.gains.stops = [0.4, 0.0, -0.3];
    let copy = scanner::on_oni_machine(&frame).unwrap();
    let stops = scanner::gain_stops(&frame);
    assert_eq!(copy.gains.stops, stops);
    assert_eq!(conversion::stage47(&copy).gain, stops.map(f32::exp2));
    assert_eq!(conversion::stage47(&copy), conversion::stage47(&scanner::on_oni_machine(&PipelineSettings { gains: Default::default(), ..frame.clone() }).unwrap()));
    // D lightens, as on a slide: every channel gains.
    let d = scanner::gain_stops(&PipelineSettings { scanner: ScannerClicks::new(4, 0, 0, 0, 0), ..frame.clone() });
    assert!(d.iter().all(|s| *s > 0.15), "{d:?}");
    // Twin: a film's own gains add to the keys, so the rule above sees the photo's stored ones left out.
    let negative = PipelineSettings { mode: ConversionMode::Negative, ..frame.clone() };
    let film = scanner::on_oni_machine(&negative).unwrap();
    assert_ne!(film.gains.stops, scanner::gain_stops(&negative));
    // On Pro's face a digital photo renders no gain.
    assert_eq!(conversion::stage47(&PipelineSettings { face: Face::Pro, ..frame }).gain, [1.0; 3]);
}

#[test]
fn a_key_is_its_stops_of_light_on_the_scene_through_the_whole_chain() {
    let Some(mut g) = graph("a_key_is_its_stops_of_light_on_the_scene_through_the_whole_chain") else { return };
    let pixels = noise(32, 16, 3, 0.02, 0.6);
    let scan = g.upload(32, 16, &pixels).unwrap();
    let rest = digital(Face::Scanner, ScannerMachine::Frontier);
    let keyed = PipelineSettings { scanner: ScannerClicks::new(4, -2, 0, 2, 0), ..rest.clone() };
    let lit = render(&mut g, &scan, &keyed, true);
    // The kernel's gain multiplies the scene with its pedestal, which the scaled scene keeps.
    let pedestal = density_axis::pedestal(density_axis::base());
    let scaled = |stops: [f32; 3]| mapped(&pixels, |p| [0, 1, 2, 3].map(|c| if c == 3 { p[3] } else { (p[c] + pedestal) * stops[c].exp2() - pedestal }));
    // The Frontier's own rest lies in both renders' gains.
    let (k, r) = (scanner::gain_stops(&keyed), scanner::gain_stops(&rest));
    let stops: [f32; 3] = std::array::from_fn(|c| k[c] - r[c]);
    let on = g.upload(32, 16, &scaled(stops)).unwrap();
    let doubled = g.upload(32, 16, &scaled(stops.map(|s| 2.0 * s))).unwrap();
    let (reference, twin) = (render(&mut g, &on, &rest, true), render(&mut g, &doubled, &rest, true));
    judge("D+4 C-2 Y+2 as stops of light on the scene", &lit, &reference, &twin);
}

#[test]
fn the_machine_prints_the_photo_at_its_as_shot_white_whatever_pros() {
    let Some(mut g) = graph("the_machine_prints_the_photo_at_its_as_shot_white_whatever_pros") else { return };
    let scan = g.upload(W, H, &scene()).unwrap();
    let shot = Some(White { kelvin: 5600.0, tint: 6.0 });
    let tungsten = Some(White { kelvin: 3200.0, tint: 0.0 });
    let frame = PipelineSettings { shot, white: shot, ..digital(Face::Scanner, ScannerMachine::Oni) };
    let warmed = PipelineSettings { white: tungsten, ..frame.clone() };
    assert_eq!(scanner::on_oni_machine(&warmed).unwrap().white, shot);
    assert_eq!(render(&mut g, &scan, &warmed, false), render(&mut g, &scan, &frame, false));
    // A plain file, no As Shot read, prints at its decode's own white.
    let plain = PipelineSettings { shot: None, ..warmed.clone() };
    assert!(conversion::white(&scanner::on_oni_machine(&plain).unwrap()).is_none());
    // Twin: on Pro the same two whites render apart.
    let pro = |s: &PipelineSettings| PipelineSettings { face: Face::Pro, ..s.clone() };
    assert_ne!(render(&mut g, &scan, &pro(&warmed), false), render(&mut g, &scan, &pro(&frame), false));
}

/// The levels and a Scanner curve moved, and keys, on the Scanner face.
fn graded() -> PipelineSettings {
    let mut s = digital(Face::Scanner, ScannerMachine::Oni);
    s.scanner = ScannerClicks::new(2, 0, 1, -2, 0);
    s.scanner_finish.contrast = 0.15;
    s.picture_levels.red.set_white(0.8);
    s.picture_levels.luma.mid = 0.4;
    s.scanner_finish.manual_curves.linked.add(Point::new(0.3, 0.22));
    s.scanner_finish.manual_curves.linked.add(Point::new(0.7, 0.8));
    // Pro's own curves wait on Pro's face.
    s.manual_curves.linked.add(Point::new(0.5, 0.9));
    s
}

/// The chain on the CPU: the picture, its levels, the machine's print, ONI's look, then the face's curves.
#[test]
fn the_levels_come_before_the_machines_print_and_the_curves_after_its_look() {
    let Some(mut g) = graph("the_levels_come_before_the_machines_print_and_the_curves_after_its_look") else { return };
    let (w, h) = (48, 32);
    let pixels = noise(w, h, 11, 0.0, 1.3);
    let scan = g.upload(w, h, &pixels).unwrap();
    let frame = graded();
    let out = render(&mut g, &scan, &frame, true);
    let copy = scanner::on_oni_machine(&frame).unwrap();
    let (head, print) = (conversion::stage47(&copy), conversion::lighting(&copy).expect("the machine's print"));
    let levels = finishing::curves(&Default::default(), Some(&copy.picture_levels)).expect("the levels");
    let curves = finishing::curves(&copy.manual_curves, None).expect("the Scanner's curves");
    let encode = ScannerEncode { root: oni_look::ROOT };
    let output = ScannerOutput { warmth_share: 0.0, lift: scanner::LIFT, contrast: 0.0 };
    let looked = |p: V4| {
        let code = cpu::color_cube(cpu::scanner_encode(p, &encode), &oni_look::CUBE);
        cpu::scanner_output(code, code, &output)
    };
    let now = |p: V4| cpu::curves(looked(cpu::print(cpu::curves(cpu::stage47(p, &head), &levels), &print)), &curves);
    // Twin: Pro's order, levels and curves together before the print.
    let both = finishing::curves(&copy.manual_curves, Some(&copy.picture_levels)).unwrap();
    let pro = |p: V4| looked(cpu::print(cpu::curves(cpu::stage47(p, &head), &both), &print));
    judge("the digital Scanner chain", &out, &mapped(&pixels, now), &mapped(&pixels, pro));
}

#[test]
fn on_pro_a_digital_photo_renders_none_of_the_scanners_values() {
    let Some(mut g) = graph("on_pro_a_digital_photo_renders_none_of_the_scanners_values") else { return };
    let scan = g.upload(W, H, &scene()).unwrap();
    let rest = digital(Face::Pro, ScannerMachine::Oni);
    let mut moved = graded();
    moved.face = Face::Pro;
    moved.manual_curves = rest.manual_curves.clone();
    moved.picture_levels = rest.picture_levels;
    moved.scanner_machine = ScannerMachine::Frontier;
    moved.scanner.w = 3;
    let bits = |px: Vec<V4>| px.iter().flatten().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(render(&mut g, &scan, &moved, false)), bits(render(&mut g, &scan, &rest, false)));
    // Twin: the same values on the Scanner face move it.
    let scanned = PipelineSettings { face: Face::Scanner, ..moved };
    assert_ne!(bits(render(&mut g, &scan, &scanned, false)), bits(render(&mut g, &scan, &rest, false)));
}

/// The machine's print rides with the encode in the picture's pass, or after the levels' table when they
/// move: the table's pass and the picture's own, which it parts from the print.
#[test]
fn the_machines_print_rides_in_the_encodes_pass() {
    let Some(mut g) = graph("the_machines_print_rides_in_the_encodes_pass") else { return };
    let scan = g.upload(W, H, &scene()).unwrap();
    let mut stages = |s: &PipelineSettings| Pipeline::new().image(&mut g, &scan, s, &Request::default(), &Inputs::default()).unwrap().stages();
    let rest = digital(Face::Scanner, ScannerMachine::Oni);
    let mut leveled = rest.clone();
    leveled.picture_levels.red.set_white(0.8);
    let (at_rest, with_levels) = (stages(&rest), stages(&leveled));
    println!("digital Scanner passes: {at_rest} at rest, {with_levels} with levels");
    // The picture, print and encode, the look, the output, the clip.
    assert_eq!((at_rest, with_levels), (4, 6));
    // Twin: a film's machine keeps its print in its picture's own pass, the encode one of its own.
    let negative = PipelineSettings { mode: ConversionMode::Negative, ..PipelineSettings::rest(ConversionMode::Negative, true).per_pixel_only() };
    assert_eq!(stages(&PipelineSettings { face: Face::Scanner, ..negative }), 5);
}
