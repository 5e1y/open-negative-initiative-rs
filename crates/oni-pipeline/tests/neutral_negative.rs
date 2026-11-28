//! The neutral film against its definition: its kernel on the GPU against its reference, its window's share a colour
//! negative's mean tone curve of the light, end to end through the pure inversion, its grey and contrast against Portra
//! 800's, and its grain uniform on the negative; each with a twin.

mod common;

use common::{V4, graph, judge, mapped, noise, whole};
use oni_graph::{Image, Rect};
use oni_pipeline::film::{self, neutral, saturation};
use oni_pipeline::{Inputs, Pipeline, Request, conversion, grain, stages};
use oni_render::{LUMA, NeutralNegative, Shown, Stage47, cpu};
use oni_settings::{ConversionMode, PaperStock, PipelineSettings, VirtualStock};

const NEUTRAL: VirtualStock = VirtualStock::NEUTRAL;

/// A new digital photo's reading at rest, before any print: stage 4's density and stage 5's windows.
fn reading() -> Stage47 {
    conversion::stage47(&PipelineSettings::rest(ConversionMode::Digital, true))
}

/// The pure inversion's light of a window's share, by the render's own kernel.
fn inverted(share: f32) -> f32 {
    cpu::inversion([share; 4])[0]
}

/// A new digital photo, its grain and its neighbourhood stages off, so each pixel renders alone.
fn plain() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true).per_pixel_only();
    s.grain.amount = 0.0;
    s
}

/// The light the pure inversion shows for a scene's light `x` over the saturation: the tone curve's share of the window.
fn toned(x: f32) -> f32 {
    inverted(neutral::kernel(NEUTRAL).expect("the neutral film").tone(x))
}

/// How far a render lies off its scene toned by the film's curve, at worst, where the scene's light is 13 stops under
/// the saturation or over.
fn off_the_scene(scenes: &[V4], rendered: &[V4]) -> f32 {
    let floor = (-13f32).exp2();
    let lights = scenes.iter().zip(rendered).flat_map(|(scene, shown)| (0..3).map(move |c| (scene[c] / saturation(), shown[c])));
    lights.filter(|(light, _)| *light >= floor && toned(*light) > 1e-4).map(|(light, shown)| (shown / toned(light) - 1.0).abs()).fold(0.0, f32::max)
}

/// Scene pixels over the sensor's whole range, each channel its own light: black, the saturation, then thirteen stops.
fn scenes() -> Vec<V4> {
    let sat = saturation();
    let mut out = vec![[0.0, 0.0, 0.0, 1.0], [sat, sat, sat, 1.0], [sat, 0.0, sat * 0.5, 1.0]];
    for i in 0..=260 {
        let at = |offset: f32| sat * (-(i as f32 + offset) / 20.0).exp2();
        out.push([at(0.0), at(3.7), at(11.3), 1.0]);
    }
    out
}

#[test]
fn a_new_digital_photo_is_shot_on_the_neutral_film_and_a_stored_one_keeps_its_own() {
    let rest = PipelineSettings::rest(ConversionMode::Digital, true);
    assert_eq!(rest.film().map(|f| f.stock), Some(NEUTRAL));
    assert!(NEUTRAL.entry().neutral && NEUTRAL.entry().offered() && NEUTRAL.white().is_none());
    assert!(conversion::scene_white(&rest).is_none(), "its scene keeps the light its decode balanced");
    assert_eq!((rest.gains.stops, rest.levels), (NEUTRAL.reference().stops, NEUTRAL.reference().levels()));
    // Twins: a photo storing no film, or a key no file defines, reads on Portra 800 as before; a data sheet's is lit.
    let stored = PipelineSettings { virtual_film: None, ..rest.clone() };
    assert_eq!(stored.film().map(|f| f.stock), Some(VirtualStock::of("kodak-portra-800")));
    assert!(!VirtualStock::of("no-such-film").entry().neutral && neutral::kernel(VirtualStock::of("kodak-portra-800")).is_none());
    assert!(conversion::scene_white(&rest.on_stock(VirtualStock::of("kodak-portra-800"))).is_some());
}

#[test]
fn the_neutral_negative_s_kernel_follows_its_reference() {
    let Some(mut g) = graph("the_neutral_negative_s_kernel_follows_its_reference") else { return };
    let (w, h) = (64, 48);
    let mut pixels = noise(w, h, 31, 0.0, 1.6);
    let edges: [V4; 8] = [
        [0.0, 0.0, 0.0, 1.0],
        [-0.2, -0.001, -0.3, 1.0],
        [30.0, 12.0, 4.0, 1.0],
        [0.003, 0.0045, 0.006, 1.0],
        [1e-6, 0.0, 1e-3, 0.5],
        [saturation(), saturation(), saturation(), 1.0],
        [0.184, 0.184, 0.184, 1.0],
        [0.3, -0.05, 2.0, 1.0],
    ];
    pixels[..edges.len()].copy_from_slice(&edges);
    let input = g.upload(w, h, &pixels).unwrap();
    let k = neutral::exposed(neutral::kernel(NEUTRAL).expect("the neutral film"), 0.4);
    let out = whole(&mut g, &Image::new(&input).then(stages::neutral_negative(&k)));
    let reference = mapped(&pixels, |p| cpu::neutral_negative(p, &k));
    let brighter = NeutralNegative { per_white: k.per_white * 1.01, ..k };
    judge("neutral negative", &out, &reference, &mapped(&pixels, |p| cpu::neutral_negative(p, &brighter)));
    judge("neutral negative, its veil kept", &out, &reference, &mapped(&pixels, |p| cpu::neutral_negative(p, &NeutralNegative { veil: 0.0, ..k })));
    judge("neutral negative, channels swapped", &out, &reference, &mapped(&reference, |p| [p[2], p[1], p[0], p[3]]));
}

#[test]
fn the_window_s_share_is_a_colour_negative_s_mean_tone_curve_of_the_light() {
    let reading = reading();
    let k = neutral::kernel(NEUTRAL).expect("the neutral film");
    let (mut worst, mut twin, mut wider) = (0.0f32, 0.0f32, 0.0f32);
    for scene in scenes() {
        let share = cpu::windowed(film::shot(NEUTRAL, scene, 0.0), &reading);
        // Twin: a film keeping the pedestal its reading adds bends its share.
        let veiled = cpu::windowed(cpu::neutral_negative(scene, &NeutralNegative { veil: 0.0, ..k }), &reading);
        for c in 0..3 {
            let tone = k.tone(scene[c] / saturation());
            worst = worst.max((share[c] - tone).abs());
            twin = twin.max((veiled[c] - tone).abs());
        }
        // The definition, on the stock's own reference: density over black, over the window.
        let (r, windows) = (NEUTRAL.reference(), NEUTRAL.reference().levels().windows());
        let scan = film::shot(NEUTRAL, scene, 0.0);
        for c in 0..3 {
            let by_definition = (film::density(scan[c], r.stops[c]) - windows[c].black) / (windows[c].white - windows[c].black);
            wider = wider.max((by_definition - share[c]).abs());
        }
    }
    println!("share against the curve: {worst:.2e}, the veiled twin {twin:.2e}, the definition against the app's reading {wider:.2e}");
    assert!(worst < 1e-4, "the share is {worst:.2e} off the curve");
    assert!(wider < 1e-5, "the app's reading is {wider:.2e} off the definition");
    assert!(twin > 2e-4, "the twin passes at {twin:.2e}");
    let ends = |scene: f32| cpu::windowed(film::shot(NEUTRAL, [scene, scene, scene, 1.0], 0.0), &reading);
    assert!(ends(0.0).iter().all(|v| v.abs() < 1e-6), "no light lies on the bare film: {:?}", ends(0.0));
    assert!(ends(saturation()).iter().all(|v| (v - 1.0).abs() < 1e-5), "the saturation lies on the window's white: {:?}", ends(saturation()));
    // The curve is its sheets': at a grey it lies where their mean density over its value at the white does.
    let grey = k.tone(0.184 / saturation());
    println!("a grey's share of the window: {grey:.3}");
    assert!((0.5..0.65).contains(&grey), "{grey}");
    // Twin: the screen's code the film once read lays the grey far lower.
    assert!(grey - cpu::screen_code(0.184 / saturation()) > 0.1);
}

/// The neutral film's profile with its range set to `range`.
fn profile_of(range: f32) -> String {
    let mut p: serde_json::Value = serde_json::from_str(&NEUTRAL.entry().profile).unwrap();
    p["data"]["range"] = serde_json::json!(range);
    p.to_string()
}

#[test]
fn the_range_moves_no_share() {
    let reading = reading();
    let narrow = neutral::read(&profile_of(0.4)).unwrap();
    let own = film::derive_neutral(&profile_of(0.4)).unwrap();
    let ends = |w: oni_settings::levels::Levels| [w.black, w.white, 1.0];
    let on_own = Stage47 { red: ends(own.levels().red), green: ends(own.levels().green), blue: ends(own.levels().blue), ..reading };
    let mut worst = 0.0f32;
    for scene in scenes() {
        let (a, b) = (cpu::windowed(film::shot(NEUTRAL, scene, 0.0), &reading), cpu::windowed(cpu::neutral_negative(scene, &narrow), &on_own));
        worst = (0..3).map(|c| (a[c] - b[c]).abs()).fold(worst, f32::max);
    }
    assert!(worst < 1e-4, "a film of another range reads {worst:.2e} off");
    // Twin: that film read on the shipped one's window lands elsewhere.
    let off = cpu::windowed(cpu::neutral_negative([0.184, 0.184, 0.184, 1.0], &narrow), &reading);
    assert!((off[1] - narrow.tone(0.184 / saturation())).abs() > 0.1);
    assert!(neutral::read(&profile_of(0.0)).is_err() && neutral::read(&profile_of(3.5)).is_err());
    let mut toe: serde_json::Value = serde_json::from_str(&profile_of(1.0)).unwrap();
    toe["data"]["toe"] = serde_json::json!(2);
    assert!(neutral::read(&toe.to_string()).is_err(), "a number the film does not read is refused");
    let mut lifted: serde_json::Value = serde_json::from_str(&profile_of(1.0)).unwrap();
    lifted["data"]["density"][0] = serde_json::json!(0.05);
    assert!(neutral::read(&lifted.to_string()).is_err(), "a curve over its bare film at no light is refused");
}

/// The displayed light of a scene grey shot on `stock` at rest, read through its pure inversion, and its contrast: the
/// stops it shows for each stop of the scene about that grey.
fn grey_and_contrast(stock: VirtualStock) -> (f32, f32) {
    let s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(stock);
    let k = conversion::stage47(&s);
    let shown = |light: f32| {
        let lit = conversion::scene_white(&s).map_or([light, light, light, 1.0], |m| cpu::color_matrix([light, light, light, 1.0], &m));
        let p = cpu::stage47(film::shot(stock, lit, 0.0), &k);
        p[0] * LUMA[0] + p[1] * LUMA[1] + p[2] * LUMA[2]
    };
    (shown(0.184), (shown(0.368) / shown(0.092)).log2() / 2.0)
}

#[test]
fn the_neutral_film_shows_a_grey_and_a_contrast_a_colour_negative_s() {
    let (neutral, portra) = (grey_and_contrast(NEUTRAL), grey_and_contrast(VirtualStock::of("kodak-portra-800")));
    let screen = (0.184 / saturation(), 1.0);
    println!(
        "a grey shown at {:.4} on the neutral film, {:.4} on Portra 800 ({:+.2} stop), the screen's mirror {:.4} ({:+.2} stop); contrast {:.3} against {:.3}, the mirror {:.3}",
        neutral.0, portra.0, (neutral.0 / portra.0).log2(), screen.0, (screen.0 / portra.0).log2(), neutral.1, portra.1, screen.1
    );
    assert!((neutral.0 / portra.0).log2().abs() < 0.5, "a grey {:+.2} stop off Portra 800's", (neutral.0 / portra.0).log2());
    assert!((neutral.1 / portra.1 - 1.0).abs() < 0.2, "a contrast {} against {}", neutral.1, portra.1);
    // Twin: the screen's mirror the film once was lays the grey more than a stop under Portra 800's.
    assert!((screen.0 / portra.0).log2() < -0.8);
}

/// End to end: a new digital photo, on the neutral film and no paper, renders its scene toned by the film's curve, through
/// the chain's CPU references and whole on the GPU. Twins: on a paper, or on Portra 800, it does not.
#[test]
fn a_photo_on_the_neutral_film_renders_its_scene_through_the_pure_inversion() {
    let s = plain();
    let k = conversion::stage47(&s);
    assert!(s.film().map(|f| f.stock) == Some(NEUTRAL) && s.paper().is_none() && k.shown == Shown::Inversion, "the neutral film, no paper");
    assert!(conversion::scene_white(&s).is_none(), "its scene lit by no light of the film's");
    let scenes = scenes();
    let through = |k: &Stage47| -> Vec<V4> { scenes.iter().map(|p| cpu::stage47(film::shot(NEUTRAL, *p, 0.0), k)).collect() };
    let on_paper = PipelineSettings { print_paper: Some(PaperStock::of("kodak-endura-premier")), ..s.clone() };
    let (cpu_off, cpu_twin) = (off_the_scene(&scenes, &through(&k)), off_the_scene(&scenes, &through(&conversion::stage47(&on_paper))));
    println!("end to end on the CPU: {:.4} %, the twin on a paper {:.0} %", cpu_off * 100.0, cpu_twin * 100.0);
    assert!(cpu_off < 2e-3, "the CPU chain renders the scene {:.3} % off", cpu_off * 100.0);
    assert!(cpu_twin > 0.05, "the paper twin passes at {cpu_twin:.2e}");
    assert!(cpu::stage47(film::shot(NEUTRAL, [0.0, 0.0, 0.0, 1.0], 0.0), &k).iter().take(3).all(|v| v.abs() < 1e-7), "no light renders black");
    let Some(mut g) = graph("a_photo_on_the_neutral_film_renders_its_scene_through_the_pure_inversion") else { return };
    let source = g.upload(scenes.len() as u32, 1, &scenes).unwrap();
    let mut render = |s: &PipelineSettings| Pipeline::new().render(&mut g, &source, s, &Request::default(), &Inputs::default(), None).unwrap().1;
    let on_portra = s.on_stock(VirtualStock::of("kodak-portra-800"));
    let (gpu_off, gpu_paper, gpu_film) = (off_the_scene(&scenes, &render(&s)), off_the_scene(&scenes, &render(&on_paper)), off_the_scene(&scenes, &render(&on_portra)));
    println!("end to end on the GPU: {:.4} %, the twins on a paper {:.0} %, on Portra 800 {:.0} %", gpu_off * 100.0, gpu_paper * 100.0, gpu_film * 100.0);
    assert!(gpu_off < 2e-3, "the GPU chain renders the scene {:.3} % off", gpu_off * 100.0);
    assert!(gpu_paper > 0.05 && gpu_film > 0.05, "the twins pass at {gpu_paper:.2e} and {gpu_film:.2e}");
}

#[test]
fn the_film_s_exposure_moves_the_light_and_nothing_else() {
    let reading = reading();
    let grey = [0.184, 0.092, 0.046, 1.0];
    let pushed = cpu::windowed(film::shot(NEUTRAL, grey, 1.0), &reading);
    let lit = cpu::windowed(film::shot(NEUTRAL, grey.map(|v| v * 2.0), 0.0), &reading);
    assert!((0..3).all(|c| (pushed[c] - lit[c]).abs() < 1e-6), "{pushed:?} against {lit:?}");
    // Twin: at rest the film reads the scene as it is, a stop under.
    let rest = cpu::windowed(film::shot(NEUTRAL, grey, 0.0), &reading);
    assert!((0..3).all(|c| rest[c] < pushed[c] - 0.02));
    // Past the saturation the film keeps developing: a highlight pushed over the white is still told apart.
    let over = |stops: f32| cpu::windowed(film::shot(NEUTRAL, [1.0, 1.0, 1.0, 1.0], stops), &reading)[1];
    assert!(over(2.0) > over(1.0) + 0.1 && over(1.0) > 1.0);
}

/// The relative spread of the pure inversion's light on a flat grey `stops` under the white, and its share's spread,
/// the film grained by `k`.
fn grained(g: &mut oni_graph::Graph, k: &oni_render::Grain, stops: f32) -> (f32, f32) {
    let (w, h) = (192u32, 192u32);
    let grey = saturation() * stops.exp2();
    let scene = Image::new(&g.upload(w, h, &vec![[grey, grey, grey, 1.0]; (w * h) as usize]).unwrap());
    let film = stages::neutral_negative(&neutral::kernel(NEUTRAL).unwrap());
    let image = grain::grained(scene.then(film), k);
    let reading = reading();
    let shares: Vec<f32> = whole(g, &image).iter().map(|p| cpu::windowed(*p, &reading)[1]).collect();
    let spread = |v: &[f32]| {
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        ((v.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / v.len() as f32).sqrt(), mean)
    };
    let lights: Vec<f32> = shares.iter().map(|s| inverted(*s)).collect();
    let (light, mean) = spread(&lights);
    (light / mean, spread(&shares).0)
}

#[test]
fn the_grain_is_uniform_on_the_negative_and_weakens_as_the_tone_rises() {
    let Some(mut g) = graph("the_grain_is_uniform_on_the_negative_and_weakens_as_the_tone_rises") else { return };
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true).per_pixel_only();
    s.grain.amount = 1.0;
    // A region of a 24 Mpx scan, its grains a few pixels wide.
    let scan = Rect::new(0, 0, 6048, 4032);
    let k = grain::grain(&s, scan, 1.0).expect("a grain");
    assert!(k.negative.is_some_and(|n| n.bare == [1.0 + n.veil; 3] && n.window.iter().all(|w| (w - n.window[0]).abs() < 1e-6)), "laid on a negative, its three layers alike on a clear bare film");
    let tones = [-7.0, -5.0, -3.0, -1.0];
    let on_negative: Vec<(f32, f32)> = tones.iter().map(|t| grained(&mut g, &k, *t)).collect();
    println!("on the negative, relative spread and share's spread from 7 stops under the white to 1: {on_negative:?}");
    assert!(on_negative.windows(2).all(|w| w[0].0 > 1.3 * w[1].0), "the grain's relative strength falls as the tone rises: {on_negative:?}");
    let shares: Vec<f32> = on_negative.iter().map(|v| v.1).collect();
    let (low, high) = (shares.iter().cloned().fold(f32::INFINITY, f32::min), shares.iter().cloned().fold(0.0, f32::max));
    assert!(high / low < 1.03, "its spread in the window's share is one at every tone: {shares:?}");
    // Twin: the same judge refuses the light's own spreads, which fall as the tone rises.
    let lights: Vec<f32> = on_negative.iter().map(|v| v.0).collect();
    assert!(lights.iter().cloned().fold(0.0, f32::max) / lights.iter().cloned().fold(f32::INFINITY, f32::min) > 1.03);
}

#[test]
fn a_real_negative_takes_no_grain() {
    let scan = Rect::new(0, 0, 600, 400);
    for mode in [ConversionMode::Negative, ConversionMode::Monochrome, ConversionMode::Positive] {
        let mut s = PipelineSettings::rest(mode, true);
        s.grain.amount = 1.0;
        assert!(grain::grain(&s, scan, 1.0).is_none(), "{mode:?}");
    }
    // Twin: a digital photo with the same Amount is grained, on its negative.
    let mut digital = PipelineSettings::rest(ConversionMode::Digital, true);
    digital.grain.amount = 1.0;
    assert!(grain::grain(&digital, scan, 1.0).is_some_and(|k| k.negative.is_some() && k.deviation[1] > 0.0));
    let luma = |p: V4| p[0] * LUMA[0] + p[1] * LUMA[1] + p[2] * LUMA[2];
    assert!(luma(film::shot(NEUTRAL, [0.1, 0.1, 0.1, 1.0], 0.0)) < luma(film::shot(NEUTRAL, [0.0, 0.0, 0.0, 1.0], 0.0)), "the film thins as its scene dims");
}

